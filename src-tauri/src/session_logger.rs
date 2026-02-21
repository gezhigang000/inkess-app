use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;

use crate::app_data_dir;

const MAX_LOG_BYTES: usize = 10 * 1024 * 1024; // 10MB per log file

pub struct SessionLogger {
    writer: BufWriter<File>,
    bytes_written: usize,
    path: PathBuf,
}

fn logs_dir() -> PathBuf {
    let dir = app_data_dir().join("inkess").join("terminal-logs");
    fs::create_dir_all(&dir).ok();
    dir
}

fn sanitize_header(s: &str) -> String {
    s.replace('\n', " ").replace('\r', "")
}

impl SessionLogger {
    pub fn new(session_id: &str, provider_name: Option<&str>, cwd: &str) -> Result<Self, String> {
        let now = Utc::now();
        let ts = now.format("%Y%m%d-%H%M%S").to_string();
        let short_id = if session_id.len() > 8 { &session_id[..8] } else { session_id };
        let filename = format!("{}-{}.log", ts, short_id);
        let path = logs_dir().join(&filename);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| format!("Failed to create log file: {}", e))?;

        let mut writer = BufWriter::new(file);
        let header = format!(
            "# version: 1\n# session: {}\n# started: {}\n# provider: {}\n# cwd: {}\n\n",
            sanitize_header(session_id),
            now.to_rfc3339(),
            sanitize_header(provider_name.unwrap_or("")),
            sanitize_header(cwd),
        );
        let header_bytes = header.as_bytes();
        writer.write_all(header_bytes).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;

        Ok(Self { writer, bytes_written: header_bytes.len(), path })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        if self.bytes_written + data.len() > MAX_LOG_BYTES {
            return Err("Log file size limit exceeded".to_string());
        }
        self.writer.write_all(data).map_err(|e| e.to_string())?;
        self.bytes_written += data.len();
        Ok(())
    }

    pub fn flush_sync(&mut self) -> Result<(), String> {
        self.writer.flush().map_err(|e| e.to_string())?;
        self.writer.get_ref().sync_data().map_err(|e| e.to_string())
    }

    pub fn close(mut self) -> Result<(), String> {
        let footer = format!("\n# closed: {}\n", Utc::now().to_rfc3339());
        self.writer.write_all(footer.as_bytes()).map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())?;
        self.writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = self.writer.get_ref().metadata() {
                let mut perms = meta.permissions();
                perms.set_mode(0o400);
                let _ = self.writer.get_ref().set_permissions(perms);
            }
        }
        Ok(())
    }

    /// Delete the log file without writing a close footer (for empty sessions).
    pub fn discard(mut self) -> Result<(), String> {
        let _ = self.writer.flush();
        let path = self.path.clone();
        drop(self);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
}

impl Drop for SessionLogger {
    fn drop(&mut self) {
        let _ = self.writer.flush();
        let _ = self.writer.get_ref().sync_data();
    }
}

pub type SharedLogger = Arc<Mutex<SessionLogger>>;
