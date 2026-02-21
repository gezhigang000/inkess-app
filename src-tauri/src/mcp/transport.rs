use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use serde_json::Value;

use super::protocol::{JsonRpcRequest, JsonRpcResponse};

pub struct StdioTransport {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: AtomicU64,
    dead: bool,
}

impl StdioTransport {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self, String> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            let mut cmd_args = vec!["/c".to_string(), format!("\"{}\"", command)];
            cmd_args.extend(args.iter().cloned());
            c.args(&cmd_args);
            c
        } else {
            let mut c = Command::new(command);
            c.args(args);
            c
        };

        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        for (k, v) in env {
            cmd.env(k, v);
        }
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn MCP server: {}", e))?;

        let stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            next_id: AtomicU64::new(1),
            dead: false,
        })
    }

    pub fn is_alive(&mut self) -> bool {
        if self.dead {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(_)) => { self.dead = true; false }
            Ok(None) => true,
            Err(_) => { self.dead = true; false }
        }
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        };

        let json = serde_json::to_string(&request)
            .map_err(|e| format!("Serialize error: {}", e))?;

        // Write request + newline
        if let Err(e) = self.stdin.write_all(json.as_bytes()).await {
            self.dead = true;
            return Err(format!("Write error: {}", e));
        }
        if let Err(e) = self.stdin.write_all(b"\n").await {
            self.dead = true;
            return Err(format!("Write error: {}", e));
        }
        if let Err(e) = self.stdin.flush().await {
            self.dead = true;
            return Err(format!("Flush error: {}", e));
        }

        // Read response with 30s timeout
        let mut line = String::new();
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.stdout.read_line(&mut line),
        )
        .await
        .map_err(|_| "MCP request timed out (30s)".to_string())?
        .map_err(|e| { self.dead = true; format!("Read error: {}", e) })?;

        if read_result == 0 {
            self.dead = true;
            return Err("MCP server closed connection".to_string());
        }

        let response: JsonRpcResponse = serde_json::from_str(line.trim())
            .map_err(|e| format!("Parse response error: {} (raw: {})", e, line.trim()))?;

        if let Some(err) = response.error {
            return Err(err.to_string());
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Send a JSON-RPC notification (no id, no response expected)
    pub async fn send_notification(&mut self, method: &str) -> Result<(), String> {
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        let json = serde_json::to_string(&notif)
            .map_err(|e| format!("Serialize error: {}", e))?;
        self.stdin.write_all(json.as_bytes()).await.map_err(|e| format!("Write error: {}", e))?;
        self.stdin.write_all(b"\n").await.map_err(|e| format!("Write error: {}", e))?;
        self.stdin.flush().await.map_err(|e| format!("Flush error: {}", e))?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), String> {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        // Best-effort reap to avoid zombie; close() should be called for proper cleanup
        let _ = self.child.try_wait();
    }
}

// --- HTTP Transport ---

pub struct HttpTransport {
    url: String,
    client: reqwest::Client,
    next_id: AtomicU64,
}

impl HttpTransport {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn is_alive(&self) -> bool {
        true // HTTP is stateless; health is checked via ping
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        };

        let resp = self.client
            .post(format!("{}/message", self.url))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP error: {}", resp.status()));
        }

        let response: JsonRpcResponse = resp.json().await
            .map_err(|e| format!("Parse response error: {}", e))?;

        if let Some(err) = response.error {
            return Err(err.to_string());
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    pub async fn send_notification(&mut self, method: &str) -> Result<(), String> {
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        let _ = self.client
            .post(format!("{}/message", self.url))
            .json(&notif)
            .send()
            .await
            .map_err(|e| format!("HTTP notification failed: {}", e))?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// --- Transport enum ---

pub enum McpTransport {
    Stdio(StdioTransport),
    Http(HttpTransport),
}

impl McpTransport {
    pub async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value, String> {
        match self {
            McpTransport::Stdio(t) => t.send_request(method, params).await,
            McpTransport::Http(t) => t.send_request(method, params).await,
        }
    }

    pub async fn send_notification(&mut self, method: &str) -> Result<(), String> {
        match self {
            McpTransport::Stdio(t) => t.send_notification(method).await,
            McpTransport::Http(t) => t.send_notification(method).await,
        }
    }

    pub async fn close(&mut self) -> Result<(), String> {
        match self {
            McpTransport::Stdio(t) => t.close().await,
            McpTransport::Http(t) => t.close().await,
        }
    }

    pub fn is_alive(&mut self) -> bool {
        match self {
            McpTransport::Stdio(t) => t.is_alive(),
            McpTransport::Http(t) => t.is_alive(),
        }
    }
}
