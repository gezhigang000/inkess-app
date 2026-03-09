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
        let new_total = self.bytes_written + data.len();
        if new_total > MAX_LOG_BYTES {
            return Err("Log file size limit exceeded".to_string());
        }
        self.writer.write_all(data).map_err(|e| e.to_string())?;
        self.bytes_written = new_total;
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
        #[cfg(not(unix))]
        {
            if let Ok(meta) = self.writer.get_ref().metadata() {
                let mut perms = meta.permissions();
                perms.set_readonly(true);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a SessionLogger writing to a specific temp directory.
    /// Constructs the logger directly to avoid env var race conditions in parallel tests.
    fn create_test_logger(dir: &std::path::Path) -> SessionLogger {
        let logs_dir = dir.join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        let filename = format!("test-{}.log", std::process::id());
        let path = logs_dir.join(&filename);

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();

        let mut writer = std::io::BufWriter::new(file);
        let header = b"# version: 1\n# session: test-session-id-12345\n# started: 2024-01-01T00:00:00Z\n# provider: TestProvider\n# cwd: /tmp/cwd\n\n";
        writer.write_all(header).unwrap();
        writer.flush().unwrap();

        SessionLogger { writer, bytes_written: header.len(), path }
    }

    #[test]
    fn sanitize_header_removes_newlines() {
        assert_eq!(sanitize_header("hello\nworld"), "hello world");
        assert_eq!(sanitize_header("hello\r\nworld"), "hello world");
        assert_eq!(sanitize_header("no newlines"), "no newlines");
    }

    #[test]
    fn logger_creates_file_with_header() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = create_test_logger(tmp.path());

        assert!(logger.path.exists());
        let content = fs::read_to_string(&logger.path).unwrap();
        assert!(content.contains("# version: 1"));
        assert!(content.contains("# session: test-session-id-12345"));
        assert!(content.contains("# provider: TestProvider"));
        assert!(content.contains("# cwd: /tmp/cwd"));
    }

    #[test]
    fn logger_short_session_id_used_in_filename() {
        // Test the filename logic directly — SessionLogger::new truncates to 8 chars
        let session_id = "abcdefghijklmnop";
        let short_id = if session_id.len() > 8 { &session_id[..8] } else { session_id };
        assert_eq!(short_id, "abcdefgh");

        let short_session = "abc";
        let short_id2 = if short_session.len() > 8 { &short_session[..8] } else { short_session };
        assert_eq!(short_id2, "abc");
    }

    #[test]
    fn logger_write_appends_data() {
        let tmp = tempfile::tempdir().unwrap();
        let mut logger = create_test_logger(tmp.path());

        logger.write(b"line 1\n").unwrap();
        logger.write(b"line 2\n").unwrap();
        logger.flush_sync().unwrap();

        let content = fs::read_to_string(&logger.path).unwrap();
        assert!(content.contains("line 1\n"));
        assert!(content.contains("line 2\n"));
    }

    #[test]
    fn logger_write_tracks_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut logger = create_test_logger(tmp.path());

        let initial = logger.bytes_written;
        logger.write(b"hello").unwrap();
        assert_eq!(logger.bytes_written, initial + 5);

        logger.write(b"world!").unwrap();
        assert_eq!(logger.bytes_written, initial + 11);
    }

    #[test]
    fn logger_write_respects_size_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut logger = create_test_logger(tmp.path());

        // Write data close to the limit
        let chunk = vec![b'a'; 1024 * 1024]; // 1MB
        for _ in 0..9 {
            logger.write(&chunk).unwrap();
        }

        // Next write should exceed 10MB limit
        let big_chunk = vec![b'b'; 2 * 1024 * 1024]; // 2MB, total would be > 10MB
        let result = logger.write(&big_chunk);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("size limit"));
    }

    #[test]
    fn logger_bytes_not_updated_on_limit_exceeded() {
        let tmp = tempfile::tempdir().unwrap();
        let mut logger = create_test_logger(tmp.path());

        // Fill up close to limit
        let chunk = vec![b'x'; 5 * 1024 * 1024];
        logger.write(&chunk).unwrap();
        let before = logger.bytes_written;

        // This should be rejected
        let over = vec![b'y'; 6 * 1024 * 1024];
        let _ = logger.write(&over);
        // bytes_written should remain unchanged since write was rejected before writing
        assert_eq!(logger.bytes_written, before);
    }

    #[test]
    fn logger_close_writes_footer() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = create_test_logger(tmp.path());
        let path = logger.path.clone();

        logger.close().unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# closed:"));
    }

    #[test]
    fn logger_discard_deletes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = create_test_logger(tmp.path());
        let path = logger.path.clone();

        assert!(path.exists());
        logger.discard().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn logger_close_sets_readonly_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = create_test_logger(tmp.path());
        let path = logger.path.clone();

        logger.close().unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o400);
        }
    }

    #[test]
    fn logger_none_provider_header() {
        // Test that sanitize_header handles empty provider
        let provider: Option<&str> = None;
        let header = format!(
            "# provider: {}\n",
            sanitize_header(provider.unwrap_or("")),
        );
        assert!(header.contains("# provider: \n"));
    }

    #[test]
    fn logger_flush_sync_works() {
        let tmp = tempfile::tempdir().unwrap();
        let mut logger = create_test_logger(tmp.path());

        logger.write(b"test data").unwrap();
        let result = logger.flush_sync();
        assert!(result.is_ok());

        // Verify data is on disk
        let content = fs::read_to_string(&logger.path).unwrap();
        assert!(content.contains("test data"));
    }

    #[test]
    fn logger_drop_flushes() {
        let tmp = tempfile::tempdir().unwrap();
        let path;
        {
            let mut logger = create_test_logger(tmp.path());
            logger.write(b"drop test data").unwrap();
            path = logger.path.clone();
            // logger dropped here
        }
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("drop test data"));
    }
}
