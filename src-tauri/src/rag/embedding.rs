use std::path::{Path, PathBuf};
use std::time::Duration;

use ort::session::{Session, builder::GraphOptimizationLevel};
use reqwest::Client;
use tauri::{AppHandle, Emitter};

const MODEL_DIR_NAME: &str = "models/all-MiniLM-L6-v2";
const MODEL_FILE: &str = "model.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";
const MODEL_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const MODEL_URL_MIRROR: &str = "https://hf-mirror.com/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
const TOKENIZER_URL_MIRROR: &str = "https://hf-mirror.com/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
const EMBEDDING_DIM: usize = 384;
const MAX_SEQ_LEN: usize = 256;
// Known file sizes as fallback when Content-Length is missing (e.g. after redirect)
const MODEL_EXPECTED_SIZE: u64 = 90_900_000; // ~87 MB
const TOKENIZER_EXPECTED_SIZE: u64 = 712_000; // ~695 KB
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(serde::Serialize, Clone)]
pub struct ModelProgress {
    pub stage: String,
    /// 0.0~1.0 for determinate, -1.0 for indeterminate (no Content-Length)
    pub progress: f64,
    /// Total bytes downloaded so far
    pub downloaded_bytes: u64,
}

pub struct EmbeddingEngine {
    session: Session,
    tokenizer: tokenizers::Tokenizer,
}

impl EmbeddingEngine {
    /// Load the ONNX model, downloading if necessary.
    pub async fn new(app: &AppHandle) -> Result<Self, String> {
        let model_dir = get_model_dir()?;
        safe_eprintln!("[rag:model] model_dir={}", model_dir.display());
        std::fs::create_dir_all(&model_dir)
            .map_err(|e| format!("Failed to create model dir: {}", e))?;

        let model_path = model_dir.join(MODEL_FILE);
        let tokenizer_path = model_dir.join(TOKENIZER_FILE);

        // Download model if not present
        if !model_path.exists() {
            safe_eprintln!("[rag:model] model not found, downloading...");
            if let Err(e) = download_file(app, MODEL_URL, &model_path, "model", MODEL_EXPECTED_SIZE).await {
                safe_eprintln!("[rag:model] primary download failed: {}, trying mirror...", e);
                download_file(app, MODEL_URL_MIRROR, &model_path, "model", MODEL_EXPECTED_SIZE).await?;
            }
            safe_eprintln!("[rag:model] model downloaded");
        } else {
            safe_eprintln!("[rag:model] model exists, size={}", std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0));
        }
        if !tokenizer_path.exists() {
            safe_eprintln!("[rag:model] tokenizer not found, downloading...");
            if let Err(e) = download_file(app, TOKENIZER_URL, &tokenizer_path, "tokenizer", TOKENIZER_EXPECTED_SIZE).await {
                safe_eprintln!("[rag:model] primary download failed: {}, trying mirror...", e);
                download_file(app, TOKENIZER_URL_MIRROR, &tokenizer_path, "tokenizer", TOKENIZER_EXPECTED_SIZE).await?;
            }
            safe_eprintln!("[rag:model] tokenizer downloaded");
        } else {
            safe_eprintln!("[rag:model] tokenizer exists");
        }

        let _ = app.emit("rag-model-progress", ModelProgress {
            stage: "loading".into(),
            progress: 0.9,
            downloaded_bytes: 0,
        });

        safe_eprintln!("[rag:model] loading ONNX session...");
        let session = Session::builder()
            .map_err(|e| format!("ONNX session builder failed: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("ONNX optimization failed: {}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("ONNX model load failed: {}", e))?;
        safe_eprintln!("[rag:model] ONNX session loaded");

        safe_eprintln!("[rag:model] loading tokenizer...");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Tokenizer load failed: {}", e))?;
        safe_eprintln!("[rag:model] tokenizer loaded");

        let _ = app.emit("rag-model-progress", ModelProgress {
            stage: "ready".into(),
            progress: 1.0,
            downloaded_bytes: 0,
        });

        Ok(Self { session, tokenizer })
    }

    /// Generate embedding for a single text.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let batch = self.embed_batch(&[text])?;
        batch.into_iter().next().ok_or_else(|| "Empty embedding result".into())
    }

    /// Generate embeddings for a batch of texts.
    pub fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let batch_size = texts.len();

        // Tokenize
        let encodings = self.tokenizer.encode_batch(texts.to_vec(), true)
            .map_err(|e| format!("Tokenization failed: {}", e))?;

        // Determine max length (capped at MAX_SEQ_LEN)
        let max_len = encodings.iter()
            .map(|e| e.get_ids().len().min(MAX_SEQ_LEN))
            .max()
            .unwrap_or(0);

        // Build input tensors: input_ids, attention_mask, token_type_ids
        let mut input_ids = vec![0i64; batch_size * max_len];
        let mut attention_mask = vec![0i64; batch_size * max_len];
        let token_type_ids = vec![0i64; batch_size * max_len];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(max_len);
            for j in 0..len {
                input_ids[i * max_len + j] = ids[j] as i64;
                attention_mask[i * max_len + j] = mask[j] as i64;
            }
        }

        let input_ids_tensor = ort::value::Value::from_array(
            ndarray::Array2::from_shape_vec((batch_size, max_len), input_ids)
                .map_err(|e| format!("input_ids shape error: {}", e))?
        ).map_err(|e| format!("input_ids tensor error: {}", e))?;

        let attention_mask_tensor = ort::value::Value::from_array(
            ndarray::Array2::from_shape_vec((batch_size, max_len), attention_mask.clone())
                .map_err(|e| format!("attention_mask shape error: {}", e))?
        ).map_err(|e| format!("attention_mask tensor error: {}", e))?;

        let token_type_ids_tensor = ort::value::Value::from_array(
            ndarray::Array2::from_shape_vec((batch_size, max_len), token_type_ids)
                .map_err(|e| format!("token_type_ids shape error: {}", e))?
        ).map_err(|e| format!("token_type_ids tensor error: {}", e))?;

        let outputs = self.session.run(
            ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor,
            ]
        ).map_err(|e| format!("ONNX inference failed: {}", e))?;

        // Output shape: [batch_size, seq_len, 384]
        // We need mean pooling with attention mask
        let output_value = &outputs[0];
        let tensor = output_value.try_extract_tensor::<f32>()
            .map_err(|e| format!("Output extraction failed: {}", e))?;
        let (_shape, data) = tensor;

        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let mut embedding = vec![0f32; EMBEDDING_DIM];
            let mut count = 0f32;
            for j in 0..max_len {
                if attention_mask[i * max_len + j] == 1 {
                    let offset = (i * max_len + j) * EMBEDDING_DIM;
                    for k in 0..EMBEDDING_DIM {
                        embedding[k] += data[offset + k];
                    }
                    count += 1.0;
                }
            }
            if count > 0.0 {
                for v in &mut embedding {
                    *v /= count;
                }
            }
            // L2 normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut embedding {
                    *v /= norm;
                }
            }
            results.push(embedding);
        }

        Ok(results)
    }
}

fn get_model_dir() -> Result<PathBuf, String> {
    let data_dir = crate::app_data_dir();
    Ok(data_dir.join("inkess").join(MODEL_DIR_NAME))
}

async fn download_file(app: &AppHandle, url: &str, dest: &Path, label: &str, expected_size: u64) -> Result<(), String> {
    safe_eprintln!("[rag:dl] start {} from {}", label, url);
    let _ = app.emit("rag-model-progress", ModelProgress {
        stage: format!("downloading_{}", label),
        progress: 0.0,
        downloaded_bytes: 0,
    });

    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    let resp = client.get(url)
        .send()
        .await
        .map_err(|e| format!("Download {} failed: {}", label, e))?;

    let status = resp.status();
    let content_length = resp.content_length();
    safe_eprintln!("[rag:dl] {} response: status={}, content_length={:?}, expected_size={}", label, status, content_length, expected_size);

    // Use Content-Length if available, otherwise fall back to expected size
    let total = content_length.unwrap_or(expected_size);
    let mut downloaded: u64 = 0;
    let mut bytes = Vec::new();
    let mut last_emitted: f64 = 0.0;

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
        downloaded += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);

        let progress = if total > 0 {
            (downloaded as f64 / total as f64).min(0.99)
        } else {
            -1.0
        };

        // Throttle: emit at most every 2% or for indeterminate every 64KB
        let should_emit = if progress >= 0.0 {
            progress - last_emitted >= 0.02 || progress >= 1.0
        } else {
            downloaded % (64 * 1024) < chunk.len() as u64
        };

        if should_emit {
            last_emitted = progress;
            let _ = app.emit("rag-model-progress", ModelProgress {
                stage: format!("downloading_{}", label),
                progress: if progress >= 0.0 { progress } else { -1.0 },
                downloaded_bytes: downloaded,
            });
        }
    }

    // Final 100% emit
    let _ = app.emit("rag-model-progress", ModelProgress {
        stage: format!("downloading_{}", label),
        progress: 1.0,
        downloaded_bytes: downloaded,
    });

    safe_eprintln!("[rag:dl] {} complete, {} bytes written to {}", label, downloaded, dest.display());
    std::fs::write(dest, &bytes)
        .map_err(|e| format!("Failed to write {}: {}", label, e))?;

    Ok(())
}
