use std::path::Path;

use rusqlite::{params, Connection};
use serde::Serialize;

/// A single search result from the vector store.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub heading: Option<String>,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub file_count: u64,
    pub chunk_count: u64,
    pub db_size_bytes: u64,
}

pub struct RagStore {
    conn: Connection,
    db_path: std::path::PathBuf,
}

impl RagStore {
    /// Open (or create) the index database at `<project_dir>/.inkess/index.db`.
    pub fn open(project_dir: &Path) -> Result<Self, String> {
        let dir = project_dir.join(".inkess");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create .inkess dir: {}", e))?;
        let db_path = dir.join("index.db");

        // Register sqlite-vec extension before opening connection
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open index db: {}", e))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("WAL mode failed: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                mtime INTEGER NOT NULL,
                hash TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                heading TEXT
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding float[384]
            );"
        ).map_err(|e| format!("Schema creation failed: {}", e))?;

        Ok(Self { conn, db_path })
    }

    /// Insert or update a file record. Returns the file id.
    pub fn upsert_file(&self, path: &str, mtime: i64, hash: &str) -> Result<i64, String> {
        // Delete old chunks + vectors first if file exists
        self.delete_file(path)?;
        self.conn.execute(
            "INSERT INTO files (path, mtime, hash) VALUES (?1, ?2, ?3)",
            params![path, mtime, hash],
        ).map_err(|e| format!("upsert_file failed: {}", e))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Insert a chunk with its embedding vector.
    pub fn insert_chunk(
        &self,
        file_id: i64,
        content: &str,
        start_line: u32,
        end_line: u32,
        heading: Option<&str>,
        embedding: &[f32],
    ) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO chunks (file_id, content, start_line, end_line, heading) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_id, content, start_line as i64, end_line as i64, heading],
        ).map_err(|e| format!("insert_chunk failed: {}", e))?;

        let chunk_id = self.conn.last_insert_rowid();
        let blob = vec_to_blob(embedding);
        self.conn.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, blob],
        ).map_err(|e| format!("insert vec failed: {}", e))?;

        Ok(())
    }

    /// Search for the top-k most similar chunks to the query vector.
    pub fn search(&self, query_vec: &[f32], top_k: usize) -> Result<Vec<SearchResult>, String> {
        let blob = vec_to_blob(query_vec);
        let mut stmt = self.conn.prepare(
            "SELECT v.chunk_id, v.distance, c.content, c.start_line, c.end_line, c.heading, f.path
             FROM vec_chunks v
             JOIN chunks c ON c.id = v.chunk_id
             JOIN files f ON f.id = c.file_id
             WHERE v.embedding MATCH ?1
             ORDER BY v.distance
             LIMIT ?2"
        ).map_err(|e| format!("search prepare failed: {}", e))?;

        let rows = stmt.query_map(params![blob, top_k as i64], |row| {
            Ok(SearchResult {
                path: row.get(6)?,
                content: row.get(2)?,
                start_line: row.get::<_, i64>(3)? as u32,
                end_line: row.get::<_, i64>(4)? as u32,
                heading: row.get(5)?,
                distance: row.get(1)?,
            })
        }).map_err(|e| format!("search query failed: {}", e))?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(|e| format!("row read failed: {}", e))?);
        }
        Ok(results)
    }

    /// Delete a file and all its chunks/vectors.
    pub fn delete_file(&self, path: &str) -> Result<usize, String> {
        // Get file id
        let file_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).ok();

        let Some(file_id) = file_id else { return Ok(0) };

        // Delete vectors for this file's chunks
        self.conn.execute(
            "DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)",
            params![file_id],
        ).map_err(|e| format!("delete vec failed: {}", e))?;

        // Delete chunks
        self.conn.execute(
            "DELETE FROM chunks WHERE file_id = ?1",
            params![file_id],
        ).map_err(|e| format!("delete chunks failed: {}", e))?;

        // Delete file record
        let deleted = self.conn.execute(
            "DELETE FROM files WHERE id = ?1",
            params![file_id],
        ).map_err(|e| format!("delete file failed: {}", e))?;

        Ok(deleted)
    }

    /// Get the stored mtime for a file path.
    pub fn get_file_mtime(&self, path: &str) -> Result<Option<i64>, String> {
        let result = self.conn.query_row(
            "SELECT mtime FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        );
        match result {
            Ok(mtime) => Ok(Some(mtime)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("get_file_mtime failed: {}", e)),
        }
    }

    /// List all indexed file paths.
    pub fn list_indexed_files(&self) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare("SELECT path FROM files")
            .map_err(|e| format!("list_indexed_files failed: {}", e))?;
        let rows = stmt.query_map([], |row| row.get(0))
            .map_err(|e| format!("list query failed: {}", e))?;
        let mut paths = Vec::new();
        for r in rows {
            paths.push(r.map_err(|e| format!("row read failed: {}", e))?);
        }
        Ok(paths)
    }

    /// Get index statistics.
    pub fn stats(&self) -> Result<IndexStats, String> {
        let file_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM files", [], |row| row.get(0),
        ).map_err(|e| format!("stats failed: {}", e))?;

        let chunk_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM chunks", [], |row| row.get(0),
        ).map_err(|e| format!("stats failed: {}", e))?;

        let db_size_bytes = std::fs::metadata(&self.db_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(IndexStats {
            file_count: file_count as u64,
            chunk_count: chunk_count as u64,
            db_size_bytes,
        })
    }

    /// Vacuum the database to reclaim space.
    pub fn vacuum(&self) -> Result<(), String> {
        self.conn.execute_batch("VACUUM;")
            .map_err(|e| format!("vacuum failed: {}", e))
    }
}

/// Convert f32 slice to little-endian byte blob for sqlite-vec.
fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
