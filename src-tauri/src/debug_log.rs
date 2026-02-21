use std::collections::VecDeque;
use std::sync::Mutex;
use serde::Serialize;

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, Serialize, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
}

pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
        }
    }

    pub fn push(&mut self, level: &str, module: &str, message: String) {
        if self.entries.len() >= MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry {
            timestamp: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
            level: level.to_string(),
            module: module.to_string(),
            message,
        });
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub static LOG_BUFFER: std::sync::LazyLock<Mutex<LogBuffer>> =
    std::sync::LazyLock::new(|| Mutex::new(LogBuffer::new()));

/// Log macro that writes to both stderr (debug) and ring buffer.
/// In release builds, stderr output is skipped.
#[macro_export]
macro_rules! app_log {
    ($level:expr, $module:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        // Always write to ring buffer
        if let Ok(mut buf) = $crate::debug_log::LOG_BUFFER.lock() {
            buf.push($level, $module, msg.clone());
        }
        // In debug builds, also write to stderr
        #[cfg(debug_assertions)]
        {
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "[{}] [{}] {}", $module, $level, msg);
        }
    }};
}

/// Convenience macros
#[macro_export]
macro_rules! app_info {
    ($module:expr, $($arg:tt)*) => { $crate::app_log!("info", $module, $($arg)*) };
}

#[macro_export]
macro_rules! app_warn {
    ($module:expr, $($arg:tt)*) => { $crate::app_log!("warn", $module, $($arg)*) };
}

#[macro_export]
macro_rules! app_error {
    ($module:expr, $($arg:tt)*) => { $crate::app_log!("error", $module, $($arg)*) };
}
