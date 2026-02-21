use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_full::notify::event::EventKind;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use tauri::{AppHandle, Emitter, Manager};

pub struct WatcherState {
    pub watcher: Mutex<Option<Debouncer<RecommendedWatcher, FileIdMap>>>,
    pub watched_path: Mutex<Option<String>>,
}

#[derive(Clone, serde::Serialize)]
struct FsChangeEvent {
    path: String,
    kind: String,
}

fn event_kind_str(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("create"),
        EventKind::Modify(_) => Some("modify"),
        EventKind::Remove(_) => Some("remove"),
        _ => None,
    }
}

#[tauri::command]
pub fn watch_directory(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<WatcherState>();
    {
        let mut w = state.watcher.lock().map_err(|e| e.to_string())?;
        *w = None;
    }
    let watch_path = Path::new(&path);
    if !watch_path.is_dir() {
        return Err("Not a valid directory".to_string());
    }
    let app_handle = app.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(1000),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    let kind_str = match event_kind_str(&event.kind) {
                        Some(k) => k,
                        None => continue,
                    };
                    for p in &event.paths {
                        let path_str = p.to_string_lossy().to_string();
                        if p.components().any(|c| c.as_os_str() == ".git") {
                            continue;
                        }
                        let _ = app_handle.emit(
                            "fs-changed",
                            FsChangeEvent { path: path_str, kind: kind_str.to_string() },
                        );
                    }
                }
            }
        },
    )
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    debouncer
        .watch(watch_path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch directory: {}", e))?;

    let mut w = state.watcher.lock().map_err(|e| e.to_string())?;
    let mut wp = state.watched_path.lock().map_err(|e| e.to_string())?;
    *wp = Some(path);
    *w = Some(debouncer);
    Ok(())
}

#[tauri::command]
pub fn unwatch_directory(app: AppHandle) -> Result<(), String> {
    let state = app.state::<WatcherState>();
    let mut w = state.watcher.lock().map_err(|e| e.to_string())?;
    *w = None;
    let mut wp = state.watched_path.lock().map_err(|e| e.to_string())?;
    *wp = None;
    Ok(())
}
