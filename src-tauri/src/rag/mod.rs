pub mod store;
pub mod embedding;
pub mod chunker;
pub mod extractor;
pub mod indexer;
pub mod cleaner;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::rag::cleaner::auto_cleanup;
use crate::rag::embedding::EmbeddingEngine;
use crate::rag::indexer::{Indexer, RagStatusEvent};
use crate::rag::store::{IndexStats, RagStore, SearchResult};

pub struct RagState {
    pub indexer: Mutex<Option<Indexer>>,
}

#[tauri::command]
pub async fn rag_init(app: AppHandle, state: State<'_, RagState>, dir: String) -> Result<(), String> {
    let dir_path = std::path::PathBuf::from(&dir);
    safe_eprintln!("[rag] init start, dir={}", dir);

    let _ = app.emit("rag-status", RagStatusEvent {
        status: "indexing".into(),
        message: "Initializing...".into(),
    });

    // Open store
    safe_eprintln!("[rag] opening store...");
    let store = RagStore::open(&dir_path)?;
    safe_eprintln!("[rag] store opened");

    // Cleanup stale entries
    let _ = auto_cleanup(&store, &dir_path);

    // Update status: downloading model if needed
    let _ = app.emit("rag-status", RagStatusEvent {
        status: "indexing".into(),
        message: "Loading model...".into(),
    });

    // Load embedding engine (downloads model if needed)
    safe_eprintln!("[rag] loading embedding engine...");
    let engine = EmbeddingEngine::new(&app).await?;
    safe_eprintln!("[rag] embedding engine ready");

    let mut indexer = Indexer::new(store, engine);

    // Update status: indexing files
    let _ = app.emit("rag-status", RagStatusEvent {
        status: "indexing".into(),
        message: "Indexing files...".into(),
    });

    // Index synchronously since rusqlite::Connection is not Send
    safe_eprintln!("[rag] indexing files...");
    let report = indexer.index_all(&dir_path, &app)?;
    safe_eprintln!("[rag] indexing done: {} files, {} chunks", report.files_indexed, report.chunks_created);

    let _ = app.emit("rag-status", RagStatusEvent {
        status: "ready".into(),
        message: format!("{} files indexed, {} chunks", report.files_indexed, report.chunks_created),
    });

    // Store indexer in state
    let mut guard = state.indexer.lock().map_err(|e| format!("Lock error: {}", e))?;
    *guard = Some(indexer);

    safe_eprintln!("[rag] init complete");
    Ok(())
}

#[tauri::command]
pub async fn rag_search(state: State<'_, RagState>, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let mut guard = state.indexer.lock().map_err(|e| format!("Lock error: {}", e))?;
    let indexer = guard.as_mut().ok_or("RAG not initialized")?;
    indexer.search(&query, top_k.unwrap_or(5))
}

#[tauri::command]
pub async fn rag_stats(state: State<'_, RagState>) -> Result<IndexStats, String> {
    let guard = state.indexer.lock().map_err(|e| format!("Lock error: {}", e))?;
    let indexer = guard.as_ref().ok_or("RAG not initialized")?;
    indexer.status()
}

#[tauri::command]
pub async fn rag_rebuild(app: AppHandle, state: State<'_, RagState>, dir: String) -> Result<(), String> {
    let dir_path = std::path::PathBuf::from(&dir);

    let _ = app.emit("rag-status", RagStatusEvent {
        status: "indexing".into(),
        message: "Rebuilding index...".into(),
    });

    // Drop old indexer
    {
        let mut guard = state.indexer.lock().map_err(|e| format!("Lock error: {}", e))?;
        *guard = None;
    }

    // Delete old database
    let db_dir = dir_path.join(".inkess");
    let db_path = db_dir.join("index.db");
    if db_path.exists() {
        std::fs::remove_file(&db_path).map_err(|e| format!("Failed to remove old index: {}", e))?;
    }

    // Re-init
    rag_init(app, state, dir).await
}
