use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize, Child};
use tauri::{AppHandle, Emitter, Manager};

use crate::session_logger::{SessionLogger, SharedLogger};

const MAX_SESSIONS: usize = 5;

const BLOCKED_ENV_VARS: &[&str] = &[
    "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH", "PROMPT_COMMAND", "ZDOTDIR", "ENV",
    "BASH_ENV", "PERL5LIB", "PYTHONPATH", "NODE_PATH",
];

pub(crate) struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    pub(crate) child: Box<dyn Child + Send + Sync>,
    logger: Option<SharedLogger>,
    flush_stop: Option<Arc<AtomicBool>>,
    has_user_input: bool,
}

pub struct PtyState {
    pub sessions: Mutex<HashMap<String, PtySession>>,
}

#[derive(Clone, serde::Serialize)]
struct PtyDataEvent {
    session_id: String,
    data: Vec<u8>,
}

#[derive(Clone, serde::Serialize)]
struct PtyExitEvent {
    session_id: String,
}

#[derive(serde::Deserialize)]
pub struct EnvVar {
    key: String,
    value: String,
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    cwd: String,
    session_id: String,
    env_vars: Option<Vec<EnvVar>>,
) -> Result<(), String> {
    safe_eprintln!("[pty] spawn: session={}, cwd={}", session_id, cwd);

    let state = app.state::<PtyState>();
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;

    if sessions.len() >= MAX_SESSIONS {
        return Err(format!("Maximum {} terminals allowed", MAX_SESSIONS));
    }
    if sessions.contains_key(&session_id) {
        return Err("Session already exists".to_string());
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to create PTY: {}", e))?;
    safe_eprintln!("[pty] openpty ok");

    let shell = if cfg!(target_os = "windows") {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };
    safe_eprintln!("[pty] shell={}", shell);
    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(&cwd);
    // Pass through common env vars
    if !cfg!(target_os = "windows") {
        cmd.env("TERM", "xterm-256color");
    }
    // Inject user-provided environment variables (with security validation)
    if let Some(vars) = &env_vars {
        safe_eprintln!("[pty] injecting {} env vars", vars.len());
        for var in vars {
            let key_upper = var.key.to_uppercase();
            if BLOCKED_ENV_VARS.contains(&key_upper.as_str()) {
                return Err(format!("Environment variable '{}' is not allowed", var.key));
            }
            if !var.key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err("Invalid environment variable name".to_string());
            }
            safe_eprintln!("[pty] env: {}={}...", var.key, &var.value[..var.value.len().min(20)]);
            cmd.env(&var.key, &var.value);
        }
    } else {
        safe_eprintln!("[pty] no env vars provided");
    }

    // Spawn the child process on the slave end
    let child = pair.slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;
    safe_eprintln!("[pty] child spawned");

    // IMPORTANT: Drop the slave immediately after spawn.
    // The slave must be dropped so the master can detect EOF when the child exits.
    drop(pair.slave);
    safe_eprintln!("[pty] slave dropped");

    let mut reader = pair.master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone reader: {}", e))?;
    safe_eprintln!("[pty] reader cloned");

    let writer = pair.master
        .take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;
    safe_eprintln!("[pty] writer taken");

    // Store the master separately for resize
    let master = pair.master;

    // Create session logger
    let provider_name: Option<String> = env_vars.as_ref().and_then(|vars| {
        if vars.is_empty() { None } else { Some(vars.iter().map(|v| format!("{}=…", v.key)).collect::<Vec<_>>().join(", ")) }
    });
    let logger: Option<SharedLogger> = SessionLogger::new(&session_id, provider_name.as_deref(), &cwd)
        .ok()
        .map(|lg| Arc::new(Mutex::new(lg)));
    let logger_clone = logger.clone();

    // Start periodic flush timer for logger with stop signal
    let flush_stop = Arc::new(AtomicBool::new(false));
    let flush_stop_clone = flush_stop.clone();
    if let Some(ref lg) = logger {
        let lg_flush = lg.clone();
        thread::spawn(move || {
            while !flush_stop_clone.load(Ordering::Relaxed) {
                thread::sleep(std::time::Duration::from_secs(2));
                if flush_stop_clone.load(Ordering::Relaxed) { break; }
                if let Ok(mut l) = lg_flush.lock() {
                    if l.flush_sync().is_err() { break; }
                } else {
                    break;
                }
            }
        });
    }

    let sid = session_id.clone();
    let app_handle = app.clone();
    thread::spawn(move || {
        safe_eprintln!("[pty] reader thread started: session={}", sid);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    safe_eprintln!("[pty] reader EOF: session={}", sid);
                    break;
                }
                Ok(n) => {
                    let data = &buf[..n];
                    let _ = app_handle.emit(
                        "pty-data",
                        PtyDataEvent {
                            session_id: sid.clone(),
                            data: data.to_vec(),
                        },
                    );
                    // Tee to logger
                    if let Some(ref lg) = logger_clone {
                        if let Ok(mut l) = lg.lock() {
                            let _ = l.write(data);
                        }
                    }
                }
                Err(e) => {
                    safe_eprintln!("[pty] reader error: session={}, err={}", sid, e);
                    break;
                }
            }
        }
        safe_eprintln!("[pty] emitting pty-exit: session={}", sid);
        let _ = app_handle.emit("pty-exit", PtyExitEvent { session_id: sid });
    });

    sessions.insert(session_id.clone(), PtySession { writer, master, child, logger, flush_stop: Some(flush_stop), has_user_input: false });
    safe_eprintln!("[pty] session stored: {}", session_id);
    Ok(())
}

#[tauri::command]
pub fn pty_write(app: AppHandle, session_id: String, data: Vec<u8>) -> Result<(), String> {
    let state = app.state::<PtyState>();
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&session_id).ok_or("Session not found")?;
    session.has_user_input = true;
    session.writer.write_all(&data).map_err(|e| format!("Write failed: {}", e))?;
    session.writer.flush().map_err(|e| format!("Flush failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn pty_resize(app: AppHandle, session_id: String, cols: u16, rows: u16) -> Result<(), String> {
    safe_eprintln!("[pty] resize: session={}, cols={}, rows={}", session_id, cols, rows);
    let state = app.state::<PtyState>();
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get(&session_id).ok_or("Session not found")?;
    session.master
        .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("Resize failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn pty_kill(app: AppHandle, session_id: String) -> Result<(), String> {
    safe_eprintln!("[pty] kill: session={}", session_id);
    let state = app.state::<PtyState>();
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(mut session) = sessions.remove(&session_id) {
        // Signal flush thread to stop
        if let Some(stop) = session.flush_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        // Brief sleep to let flush thread exit
        thread::sleep(std::time::Duration::from_millis(100));
        // Close or discard logger based on whether user typed anything
        if let Some(logger) = session.logger.take() {
            if let Ok(lg) = Arc::try_unwrap(logger) {
                if let Ok(lg) = lg.into_inner() {
                    if session.has_user_input {
                        let _ = lg.close();
                    } else {
                        let _ = lg.discard();
                    }
                }
            }
        }
        // Kill the child process and wait for it to avoid zombie
        let _ = session.child.kill();
        let _ = session.child.wait();
        safe_eprintln!("[pty] child killed: {}", session_id);
    }
    Ok(())
}
