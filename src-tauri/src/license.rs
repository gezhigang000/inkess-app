use std::fs;
use std::path::PathBuf;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

// HMAC key segments injected at build time from security/hmac_keys.rs (private repo)
// or generated as placeholders for public/open-source builds
include!(concat!(env!("OUT_DIR"), "/hmac_keys.rs"));

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

#[cfg(test)]
mod tests {
    use super::*;

    // --- Key format validation tests ---

    #[test]
    fn verify_key_wrong_prefix() {
        assert!(!verify_key("WRONG-AAAA-BBBB-CCCC-DDDD"));
    }

    #[test]
    fn verify_key_too_few_segments() {
        assert!(!verify_key("INKESS-AAAA-BBBB-CCCC"));
    }

    #[test]
    fn verify_key_too_many_segments() {
        assert!(!verify_key("INKESS-AAAA-BBBB-CCCC-DDDD-EEEE"));
    }

    #[test]
    fn verify_key_empty_string() {
        assert!(!verify_key(""));
    }

    #[test]
    fn verify_key_segment_too_short() {
        assert!(!verify_key("INKESS-AA-BBBB-CCCC-DDDD"));
    }

    #[test]
    fn verify_key_segment_too_long() {
        assert!(!verify_key("INKESS-AAAAA-BBBB-CCCC-DDDD"));
    }

    #[test]
    fn verify_key_wrong_checksum() {
        // Valid format but wrong checksum — should fail HMAC verification
        assert!(!verify_key("INKESS-AAAA-BBBB-CCCC-ZZZZ"));
    }

    #[test]
    fn verify_key_all_segments_must_be_4_chars() {
        assert!(!verify_key("INKESS-ABC-BBBB-CCCC-DDDD"));
        assert!(!verify_key("INKESS-AAAA-B-CCCC-DDDD"));
        assert!(!verify_key("INKESS-AAAA-BBBB-C-DDDD"));
        assert!(!verify_key("INKESS-AAAA-BBBB-CCCC-D"));
    }

    #[test]
    fn verify_key_correct_checksum_with_dev_keys() {
        // Generate a valid key using the dev/placeholder HMAC keys
        let payload = "INKESS-TEST-ABCD-EF01";
        let key = hmac_key();
        let mut mac = HmacSha256::new_from_slice(&key).unwrap();
        mac.update(payload.as_bytes());
        let result = mac.finalize().into_bytes();
        let checksum = hex::encode(result);
        let check_segment = &checksum[..4].to_uppercase();

        let full_key = format!("{}-{}", payload, check_segment);
        assert!(verify_key(&full_key));
    }

    #[test]
    fn verify_key_case_insensitive_checksum() {
        // Generate a valid key and verify with different case
        let payload = "INKESS-CAFE-BABE-DEAD";
        let key = hmac_key();
        let mut mac = HmacSha256::new_from_slice(&key).unwrap();
        mac.update(payload.as_bytes());
        let result = mac.finalize().into_bytes();
        let checksum = hex::encode(result);
        let check_segment = &checksum[..4].to_lowercase();

        let full_key = format!("{}-{}", payload, check_segment);
        assert!(verify_key(&full_key));
    }

    #[test]
    fn verify_key_different_payload_different_checksum() {
        // Two different payloads should not have the same checksum
        let payload1 = "INKESS-AAAA-BBBB-CCCC";
        let payload2 = "INKESS-XXXX-YYYY-ZZZZ";
        let key = hmac_key();

        let mut mac1 = HmacSha256::new_from_slice(&key).unwrap();
        mac1.update(payload1.as_bytes());
        let check1 = hex::encode(mac1.finalize().into_bytes())[..4].to_uppercase();

        let mut mac2 = HmacSha256::new_from_slice(&key).unwrap();
        mac2.update(payload2.as_bytes());
        let check2 = hex::encode(mac2.finalize().into_bytes())[..4].to_uppercase();

        // Using checksum from payload1 with payload2 should fail
        let wrong_key = format!("{}-{}", payload2, check1);
        if check1 != check2 {
            assert!(!verify_key(&wrong_key));
        }
    }

    #[test]
    fn hmac_key_is_not_empty() {
        let key = hmac_key();
        assert!(!key.is_empty());
    }

    // --- LicenseInfo serialization ---

    #[test]
    fn license_info_roundtrip_json() {
        let info = LicenseInfo {
            key: "INKESS-AAAA-BBBB-CCCC-DDDD".to_string(),
            activated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: LicenseInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, info.key);
        assert_eq!(parsed.activated_at, info.activated_at);
    }
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
