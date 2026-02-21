use std::path::Path;

fn main() {
    // Copy HMAC keys from security/ (private repo) or generate placeholders (public repo)
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let security_keys = Path::new(&manifest_dir).join("../../security/hmac_keys.rs");

    let dest = Path::new(&out_dir).join("hmac_keys.rs");
    if security_keys.exists() {
        std::fs::copy(&security_keys, &dest).expect("Failed to copy hmac_keys.rs");
    } else {
        // Placeholder for public/open-source builds
        std::fs::write(
            &dest,
            r#"const HMAC_KEY_A: &[u8] = b"PLACEHOLDER_KEY_SEGMENT_A";
const HMAC_KEY_B: &[u8] = b"PLACEHOLDER_KEY_B";
const HMAC_KEY_C: &[u8] = b"PLACEHOLDER_KEY_SEGMENT_C";
"#,
        )
        .expect("Failed to write placeholder hmac_keys.rs");
    }

    println!("cargo:rerun-if-changed=../../security/hmac_keys.rs");
    tauri_build::build()
}
