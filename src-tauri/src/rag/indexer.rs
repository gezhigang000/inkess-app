use std::path::Path;
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::rag::chunker;
use crate::rag::embedding::EmbeddingEngine;
use crate::rag::extractor;
use crate::rag::store::{IndexStats, RagStore, SearchResult};

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexReport {
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub chunks_created: usize,
}

#[derive(serde::Serialize, Clone)]
pub struct RagStatusEvent {
    pub status: String, // "indexing" | "ready" | "error"
    pub message: String,
}

pub struct Indexer {
    store: RagStore,
    engine: EmbeddingEngine,
}

impl Indexer {
    pub fn new(store: RagStore, engine: EmbeddingEngine) -> Self {
        Self { store, engine }
    }

    /// Index all eligible files in a directory.
    pub fn index_all(&mut self, dir: &Path, app: &AppHandle) -> Result<IndexReport, String> {
        let mut files_indexed = 0usize;
        let mut files_skipped = 0usize;
        let mut chunks_created = 0usize;

        let entries = collect_files(dir, dir)?;
        let total = entries.len();
        safe_eprintln!("[rag:index] found {} files to process", total);

        for (i, (full_path, rel_path)) in entries.iter().enumerate() {
            // Check if file needs re-indexing by mtime
            let mtime = get_mtime(full_path)?;
            if let Ok(Some(stored_mtime)) = self.store.get_file_mtime(rel_path) {
                if stored_mtime == mtime {
                    files_skipped += 1;
                    continue;
                }
            }

            match self.index_single_file(full_path, rel_path) {
                Ok(n) => {
                    files_indexed += 1;
                    chunks_created += n;
                }
                Err(e) => {
                    safe_eprintln!("RAG: skip {}: {}", rel_path, e);
                    files_skipped += 1;
                }
            }

            // Emit progress every 3 files
            if i % 3 == 0 || i + 1 == total {
                let _ = app.emit("rag-status", RagStatusEvent {
                    status: "indexing".into(),
                    message: format!("{}/{} files", i + 1, total),
                });
            }
        }

        safe_eprintln!("[rag:index] done: indexed={}, skipped={}, chunks={}", files_indexed, files_skipped, chunks_created);
        Ok(IndexReport {
            files_indexed,
            files_skipped,
            chunks_created,
        })
    }

    /// Index a single file. Returns number of chunks created.
    fn index_single_file(&mut self, full_path: &Path, rel_path: &str) -> Result<usize, String> {
        let (content, file_type) = extractor::extract_text(full_path)?;

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let mtime = get_mtime(full_path)?;
        let file_id = self.store.upsert_file(rel_path, mtime, &hash)?;

        let chunks = chunker::chunk_text(&content, file_type);
        if chunks.is_empty() {
            return Ok(0);
        }

        // Batch embed
        let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();

        // Process in batches of 32 to avoid OOM
        let batch_size = 32;
        let mut chunk_idx = 0;
        for batch_start in (0..texts.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(texts.len());
            let batch = &texts[batch_start..batch_end];

            let embeddings = self.engine.embed_batch(batch)?;

            for (j, embedding) in embeddings.iter().enumerate() {
                let chunk = &chunks[batch_start + j];
                self.store.insert_chunk(
                    file_id,
                    &chunk.content,
                    chunk.start_line,
                    chunk.end_line,
                    chunk.heading.as_deref(),
                    embedding,
                )?;
                chunk_idx += 1;
            }
        }

        Ok(chunk_idx)
    }

    /// Search the index.
    pub fn search(&mut self, query: &str, top_k: usize) -> Result<Vec<SearchResult>, String> {
        let query_vec = self.engine.embed(query)?;
        self.store.search(&query_vec, top_k)
    }

    /// Get index statistics.
    pub fn status(&self) -> Result<IndexStats, String> {
        self.store.stats()
    }
}

/// Recursively collect files that should be indexed, returning (full_path, relative_path).
fn collect_files(dir: &Path, base: &Path) -> Result<Vec<(std::path::PathBuf, String)>, String> {
    let mut result = Vec::new();
    collect_files_recursive(dir, base, &mut result, 0)?;
    Ok(result)
}

fn collect_files_recursive(
    dir: &Path,
    base: &Path,
    result: &mut Vec<(std::path::PathBuf, String)>,
    depth: u32,
) -> Result<(), String> {
    if depth > 8 {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read dir: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            // Skip hidden dirs and known skip dirs
            if name.starts_with('.') || extractor::SKIP_DIRS.contains(&name) {
                continue;
            }
            collect_files_recursive(&path, base, result, depth + 1)?;
        } else if extractor::should_index(&path) {
            let rel = path.strip_prefix(base)
                .map_err(|_| "strip_prefix failed".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            result.push((path.clone(), rel));
        }
    }

    Ok(())
}

fn get_mtime(path: &Path) -> Result<i64, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("Cannot stat file: {}", e))?;
    let mtime = meta.modified()
        .map_err(|e| format!("Cannot get mtime: {}", e))?;
    let duration = mtime.duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("mtime conversion error: {}", e))?;
    Ok(duration.as_secs() as i64)
}
