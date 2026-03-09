use std::fs;
use std::path::PathBuf;
use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::python_setup;

pub struct RunPythonTool;

#[async_trait]
impl ToolPlugin for RunPythonTool {
    fn name(&self) -> &str { "run_python" }
    fn description(&self) -> &str { "Execute Python code (embedded standalone Python, 30s timeout). Pre-installed: numpy, matplotlib, pandas, scipy, sympy, Pillow, openpyxl. Can read/write local files. For large files: read a sample first, then process in chunks across multiple calls." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" }
            },
            "required": ["code"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let code = input["code"].as_str().unwrap_or("");

        // Validate code against sandbox rules
        let sandbox = crate::ai::sandbox::SandboxConfig::new(&ctx.workspace_path);
        if let Err(msg) = sandbox.validate_code(code) {
            return Ok(ToolOutput::error(format!("Code blocked for security: {}", msg)));
        }

        // Prepend sandbox preamble
        let full_code = format!("{}\n{}", sandbox.preamble(), code);
        let result = run_python(&full_code, &ctx.app_handle, &ctx.workspace_path).await;
        Ok(ToolOutput::success(result))
    }
}

fn find_python() -> Option<PathBuf> {
    let p = python_setup::python_bin_path();
    if p.exists() { Some(p) } else { None }
}

async fn run_python(code: &str, app: &AppHandle, cwd: &str) -> String {
    if code.trim().is_empty() {
        return "Please provide Python code to execute".to_string();
    }

    let python_path = match find_python() {
        Some(p) => p,
        None => {
            // Auto-trigger Python environment setup
            match python_setup::setup_python_env(app).await {
                Ok(p) => p,
                Err(e) => return format!("Python environment setup failed: {}", e),
            }
        }
    };

    // Write code to a temp file
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join(format!("inkess_py_{}.py", uuid::Uuid::new_v4()));
    if let Err(e) = fs::write(&tmp_file, code) {
        return format!("Failed to write temp file: {}", e);
    }

    // Execute with 30s timeout — spawn explicitly so we can kill on timeout
    let mut child = {
        let mut cmd = tokio::process::Command::new(&python_path);
        cmd.arg(&tmp_file);
        cmd.env("PYTHONIOENCODING", "utf-8");
        cmd.env("PYTHONUNBUFFERED", "1");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if !cwd.is_empty() {
            cmd.current_dir(cwd);
        }
        match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&tmp_file);
                return format!("Failed to start Python process: {}", e);
            }
        }
    };

    // Take stdout/stderr before waiting — read concurrently to avoid pipe buffer deadlock
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Spawn tasks to drain stdout/stderr concurrently with child.wait()
    let stdout_task = tokio::spawn(async move {
        if let Some(mut h) = stdout_handle {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });
    let stderr_task = tokio::spawn(async move {
        if let Some(mut h) = stderr_handle {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });

    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        child.wait(),
    )
    .await;

    // On timeout, explicitly kill the child process
    if wait_result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    // Clean up temp file
    let _ = fs::remove_file(&tmp_file);

    // Collect output from drain tasks
    let stdout_str = stdout_task.await.unwrap_or_default();
    let stderr_str = stderr_task.await.unwrap_or_default();

    match wait_result {
        Ok(Ok(status)) => {
            // Clean stderr: replace temp file paths with "<script>" for readability
            let stderr = {
                let mut s = stderr_str.clone();
                // Remove full temp file paths like /tmp/inkess_py_xxxx.py
                // Use string find/replace instead of byte-level slicing to avoid CJK char boundary issues
                while let Some(start) = s.find("inkess_py_") {
                    if let Some(rel_end) = s[start..].find(".py") {
                        let end = start + rel_end + 3; // after ".py"
                        // Find the start of the path (preceding quote, space, or newline)
                        let prefix_start = s[..start]
                            .rfind(|c: char| c == '"' || c == '\'' || c == ' ' || c == '\n')
                            .map(|i| {
                                // Advance past the delimiter char — must be on a char boundary
                                let mut next = i + 1;
                                while next < s.len() && !s.is_char_boundary(next) {
                                    next += 1;
                                }
                                next
                            })
                            .unwrap_or(0);
                        s = format!("{}<script>{}", &s[..prefix_start], &s[end..]);
                    } else {
                        break;
                    }
                }
                // Filter out non-UTF8 replacement chars
                s.replace('\u{FFFD}', "?")
            };
            if status.success() {
                if stdout_str.is_empty() && stderr.is_empty() {
                    "(execution successful, no output)".to_string()
                } else if stderr.is_empty() {
                    stdout_str
                } else {
                    format!("{}\n[stderr]: {}", stdout_str, stderr)
                }
            } else {
                if stderr.is_empty() {
                    format!("Python execution failed (exit code: {:?})\n{}", status.code(), stdout_str)
                } else {
                    format!("Python execution failed:\n{}", stderr)
                }
            }
        }
        Ok(Err(e)) => format!("Python execution error: {}", e),
        Err(_) => "Python execution timed out (30s limit). The process has been terminated.".to_string(),
    }
}
