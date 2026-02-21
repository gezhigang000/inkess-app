use std::fs;
use std::path::PathBuf;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

// Must match LICENSE_HMAC_KEY in Cloudflare Worker
// Replace with your own HMAC key segments for license verification
const HMAC_KEY_A: &[u8] = b"YOUR_HMAC_KEY_SEGMENT_A_HERE";
const HMAC_KEY_B: &[u8] = b"YOUR_HMAC_KEY_SEGMENT_B_HERE";
const HMAC_KEY_C: &[u8] = b"YOUR_HMAC_KEY_SEGMENT_C_HERE";

fn hmac_key() -> Vec<u8> {
    let mut key = Vec::with_capacity(HMAC_KEY_A.len() + HMAC_KEY_B.len() + HMAC_KEY_C.len());
    key.extend_from_slice(HMAC_KEY_A);
    key.extend_from_slice(HMAC_KEY_B);
    key.extend_from_slice(HMAC_KEY_C);
    key
}

fn license_path() -> PathBuf {
    let data_dir = crate::app_data_dir();
    let dir = data_dir.join("inkess");
    fs::create_dir_all(&dir).ok();
    dir.join("license.json")
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LicenseInfo {
    pub key: String,
    pub activated_at: String,
}

/// Verify key format: INKESS-XXXX-XXXX-XXXX-CCCC
/// First 3 segments are random, last segment is HMAC checksum
fn verify_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() != 5 || parts[0] != "INKESS" {
        return false;
    }
    // Each segment should be 4 chars
    if parts[1..].iter().any(|p| p.len() != 4) {
        return false;
    }

    // Recompute HMAC of "INKESS-seg1-seg2-seg3" and check last segment
    let payload = format!("INKESS-{}-{}-{}", parts[1], parts[2], parts[3]);
    let key = hmac_key();
    let Ok(mut mac) = HmacSha256::new_from_slice(&key) else {
        return false;
    };
    mac.update(payload.as_bytes());
    let result = mac.finalize().into_bytes();
    let checksum = hex::encode(result);
    let expected = &checksum[..4].to_uppercase();

    parts[4].to_uppercase() == *expected
}

#[tauri::command]
pub fn license_load() -> Option<LicenseInfo> {
    let path = license_path();
    let data = fs::read_to_string(&path).ok()?;
    let info: LicenseInfo = serde_json::from_str(&data).ok()?;
    if verify_key(&info.key) {
        Some(info)
    } else {
        None
    }
}

#[tauri::command]
pub fn license_activate(key: String) -> Result<LicenseInfo, String> {
    let key = key.trim().to_uppercase();

    if !verify_key(&key) {
        return Err("Invalid License Key".to_string());
    }

    let info = LicenseInfo {
        key: key.clone(),
        activated_at: chrono::Utc::now().to_rfc3339(),
    };

    let path = license_path();
    let json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Failed to save license: {}", e))?;

    Ok(info)
}

#[tauri::command]
pub fn license_deactivate() -> Result<(), String> {
    let path = license_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to remove license: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // Use explorer.exe instead of cmd /C start to avoid URL mangling
        // (cmd.exe treats & as command separator and mishandles quoted args with start)
        std::process::Command::new("explorer").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}
