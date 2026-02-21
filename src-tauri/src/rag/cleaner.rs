use std::path::Path;

use crate::rag::store::RagStore;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanupReport {
    pub files_removed: usize,
    pub chunks_removed: usize,
    pub vacuumed: bool,
}

/// Remove index entries for files that no longer exist on disk.
pub fn auto_cleanup(store: &RagStore, project_dir: &Path) -> Result<CleanupReport, String> {
    let indexed = store.list_indexed_files()?;
    let mut files_removed = 0usize;
    let mut chunks_removed = 0usize;

    for rel_path in &indexed {
        let full_path = project_dir.join(rel_path);
        if !full_path.exists() {
            let deleted = store.delete_file(rel_path)?;
            chunks_removed += deleted;
            files_removed += 1;
        }
    }

    let vacuumed = files_removed > 100;
    if vacuumed {
        store.vacuum()?;
    }

    Ok(CleanupReport {
        files_removed,
        chunks_removed,
        vacuumed,
    })
}
