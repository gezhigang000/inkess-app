use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

static SETUP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

const PBS_VERSION: &str = "20241219";
const PYTHON_VERSION: &str = "3.12.8";
const BASE_URL: &str = "https://github.com/indygreg/python-build-standalone/releases/download";

#[derive(Serialize, Clone, Debug)]
pub struct PythonSetupProgress {
    pub status: String,
    pub progress: f64,
    pub message: String,
}

/// Returns the python-standalone directory under user data dir
pub fn python_env_dir() -> PathBuf {
    let data_dir = crate::app_data_dir();
    let dir = data_dir.join("inkess").join("python-standalone");
    dir
}

/// Returns the python binary path (platform-specific)
pub fn python_bin_path() -> PathBuf {
    let base = python_env_dir();
    #[cfg(target_os = "windows")]
    { base.join("python.exe") }
    #[cfg(not(target_os = "windows"))]
    { base.join("bin").join("python3") }
}

/// Check if Python environment is already installed
pub fn is_python_installed() -> bool {
    python_bin_path().exists()
}

fn emit_progress(app: &AppHandle, status: &str, progress: f64, message: &str) {
    let _ = app.emit("python-setup-progress", PythonSetupProgress {
        status: status.into(),
        progress,
        message: message.into(),
    });
}

fn get_download_url() -> Result<String, String> {
    let filename = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        format!("cpython-{PYTHON_VERSION}+{PBS_VERSION}-aarch64-apple-darwin-install_only.tar.gz")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        format!("cpython-{PYTHON_VERSION}+{PBS_VERSION}-x86_64-apple-darwin-install_only.tar.gz")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        format!("cpython-{PYTHON_VERSION}+{PBS_VERSION}-x86_64-pc-windows-msvc-install_only.tar.gz")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        format!("cpython-{PYTHON_VERSION}+{PBS_VERSION}-x86_64-unknown-linux-gnu-install_only.tar.gz")
    } else {
        return Err(format!("Unsupported platform: {} {}", std::env::consts::OS, std::env::consts::ARCH));
    };
    Ok(format!("{BASE_URL}/{PBS_VERSION}/{filename}"))
}

/// RAII guard to reset SETUP_IN_PROGRESS on drop (including panic)
struct SetupGuard;
impl Drop for SetupGuard {
    fn drop(&mut self) {
        SETUP_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

/// Download, extract, and install packages. Returns python binary path.
pub async fn setup_python_env(app: &AppHandle) -> Result<PathBuf, String> {
    if SETUP_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("Python environment setup already in progress".into());
    }
    let _guard = SetupGuard;
    do_setup(app).await
}

async fn do_setup(app: &AppHandle) -> Result<PathBuf, String> {
    let url = get_download_url()?;
    let env_dir = python_env_dir();
    let parent = env_dir.parent().unwrap_or(&env_dir);
    fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;

    // --- Step 1: Download tar.gz ---
    emit_progress(app, "downloading", 0.0, "Downloading Python runtime...");

    let client = Client::new();
    let resp = client.get(&url)
        .send()
        .await
        .map_err(|e| {
            emit_progress(app, "error", 0.0, &format!("Download failed: {}", e));
            format!("Download failed: {}", e)
        })?;

    if !resp.status().is_success() {
        let msg = format!("Download failed: HTTP {}", resp.status());
        emit_progress(app, "error", 0.0, &msg);
        return Err(msg);
    }

    let total_size = resp.content_length().unwrap_or(0);
    let tmp_file = parent.join("python-download.tar.gz");
    let mut file = fs::File::create(&tmp_file)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrupted: {}", e))?;
        std::io::Write::write_all(&mut file, &chunk)
            .map_err(|e| format!("Write failed: {}", e))?;
        downloaded += chunk.len() as u64;
        if total_size > 0 {
            let pct = (downloaded as f64 / total_size as f64) * 0.70;
            emit_progress(app, "downloading", pct, &format!(
                "Downloading Python runtime... {:.0}MB / {:.0}MB",
                downloaded as f64 / 1_048_576.0,
                total_size as f64 / 1_048_576.0,
            ));
        }
    }
    drop(file);

    // --- Step 2: Extract tar.gz ---
    emit_progress(app, "extracting", 0.70, "Extracting Python runtime...");

    // Remove old installation if exists
    if env_dir.exists() {
        let _ = fs::remove_dir_all(&env_dir);
    }

    {
        let tar_gz = fs::File::open(&tmp_file)
            .map_err(|e| format!("Failed to open archive: {}", e))?;
        let decompressor = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(decompressor);

        // python-build-standalone extracts to "python/" directory
        // We need to remap it to our target dir
        let extract_parent = parent;
        archive.unpack(extract_parent)
            .map_err(|e| {
                emit_progress(app, "error", 0.70, &format!("Extraction failed: {}", e));
                format!("Extraction failed: {}", e)
            })?;
    }

    // Rename "python" -> "python-standalone"
    let extracted = parent.join("python");
    if extracted.exists() && !env_dir.exists() {
        fs::rename(&extracted, &env_dir)
            .map_err(|e| format!("Failed to rename directory: {}", e))?;
    }

    // Clean up tarball
    let _ = fs::remove_file(&tmp_file);

    emit_progress(app, "extracting", 0.80, "Extraction complete");

    // --- Step 3: Install scientific packages ---
    emit_progress(app, "installing_packages", 0.80, "Installing scientific packages...");

    let python = python_bin_path();
    if !python.exists() {
        let msg = "Python executable not found after extraction";
        emit_progress(app, "error", 0.80, msg);
        return Err(msg.into());
    }

    let packages = ["numpy", "matplotlib", "pandas", "scipy", "sympy", "Pillow", "openpyxl", "seaborn"];
    let output = tokio::process::Command::new(&python)
        .args(["-m", "pip", "install", "--no-warn-script-location"])
        .args(&packages)
        .output()
        .await
        .map_err(|e| {
            emit_progress(app, "error", 0.85, &format!("Package install failed: {}", e));
            format!("pip install failed: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!("pip install failed: {}", stderr);
        emit_progress(app, "error", 0.90, &msg);
        return Err(msg);
    }

    emit_progress(app, "done", 1.0, "Python environment ready");

    Ok(python_bin_path())
}

/// Background preload: install Python env if not already installed.
#[tauri::command]
pub async fn preload_python_env(app: AppHandle) -> Result<(), String> {
    if is_python_installed() {
        safe_eprintln!("[python] already installed, skip preload");
        return Ok(());
    }
    safe_eprintln!("[python] preloading python env in background...");
    setup_python_env(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn check_python_env() -> Result<serde_json::Value, String> {
    let installed = is_python_installed();
    let path = if installed {
        Some(python_bin_path().to_string_lossy().to_string())
    } else {
        None
    };
    Ok(serde_json::json!({
        "installed": installed,
        "path": path,
    }))
}
