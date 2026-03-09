use std::time::Duration;
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;
use tauri::Emitter;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};

pub struct RunShellTool;

// Command security tiers
const AUTO_ALLOW: &[&str] = &[
    "ls", "cat", "head", "tail", "wc", "file", "which", "echo", "pwd", "env", "date",
    "find", "tree", "du", "df", "uname", "whoami", "hostname",
    "git log", "git status", "git diff", "git branch", "git remote", "git show", "git tag",
];

const NEED_CONFIRM: &[&str] = &[
    "mv", "cp", "mkdir", "touch", "chmod", "chown", "ln",
    "git add", "git commit", "git checkout", "git merge", "git rebase", "git stash",
    "npm install", "npm run", "yarn", "pnpm", "pip install", "cargo build", "cargo run",
    "make", "cmake",
];

const BLOCKED: &[&str] = &[
    "rm", "git push", "git reset", "git clean", "git force",
    "kill", "killall", "pkill",
    "dd", "mkfs", "fdisk", "mount", "umount",
    "sudo", "su ",
    "curl|sh", "curl|bash", "wget|sh", "wget|bash",
    "shutdown", "reboot", "halt",
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum CommandTier {
    AutoAllow,
    NeedConfirm,
    Blocked,
}

fn classify_command(cmd: &str) -> CommandTier {
    let trimmed = cmd.trim();

    // Commands containing backticks or $() subshells can execute arbitrary code
    if trimmed.contains('`') || trimmed.contains("$(") {
        // At minimum NeedConfirm; check sub-parts for Blocked
        let inner_tier = classify_single(trimmed);
        return if inner_tier > CommandTier::NeedConfirm { inner_tier } else { CommandTier::NeedConfirm };
    }

    // Split on all command chaining operators: |, &&, ||, ;
    // Use the highest risk level across all sub-commands
    let parts = split_command_operators(trimmed);
    if parts.len() > 1 {
        let mut max_tier = CommandTier::AutoAllow;
        for part in &parts {
            let tier = classify_single(part.trim());
            if tier > max_tier {
                max_tier = tier;
            }
        }
        return max_tier;
    }

    classify_single(trimmed)
}

/// Split a command string on shell operators: |, &&, ||, ;
fn split_command_operators(cmd: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut last = 0;
    let bytes = cmd.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'|' {
            if i + 1 < len && bytes[i + 1] == b'|' {
                // ||
                parts.push(&cmd[last..i]);
                i += 2;
                last = i;
            } else {
                // |
                parts.push(&cmd[last..i]);
                i += 1;
                last = i;
            }
        } else if bytes[i] == b'&' && i + 1 < len && bytes[i + 1] == b'&' {
            // &&
            parts.push(&cmd[last..i]);
            i += 2;
            last = i;
        } else if bytes[i] == b';' {
            parts.push(&cmd[last..i]);
            i += 1;
            last = i;
        } else {
            i += 1;
        }
    }
    if last < len {
        parts.push(&cmd[last..]);
    }
    parts
}

fn classify_single(cmd: &str) -> CommandTier {
    let trimmed = cmd.trim();

    // Extract the base command name: strip path prefix and common wrappers
    let base_cmd = extract_base_command(trimmed);

    // Check blocked first (highest priority)
    for pattern in BLOCKED {
        if base_cmd.starts_with(pattern) || base_cmd.contains(&format!(" {}", pattern)) {
            return CommandTier::Blocked;
        }
    }
    // Check need_confirm
    for pattern in NEED_CONFIRM {
        if base_cmd.starts_with(pattern) {
            return CommandTier::NeedConfirm;
        }
    }
    // Check auto_allow
    for pattern in AUTO_ALLOW {
        if base_cmd.starts_with(pattern) {
            return CommandTier::AutoAllow;
        }
    }
    // Unknown commands need confirmation by default
    CommandTier::NeedConfirm
}

/// Extract the base command from a potentially path-prefixed or wrapper-prefixed command.
/// E.g. "/usr/bin/sudo rm" -> "sudo rm", "env rm -rf /" -> "rm -rf /",
/// "/bin/rm -rf /" -> "rm -rf /"
fn extract_base_command(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
    if parts.is_empty() {
        return cmd.to_string();
    }

    // Strip directory prefix from the first token
    let first = parts[0];
    let base_name = if let Some(pos) = first.rfind('/') {
        &first[pos + 1..]
    } else if let Some(pos) = first.rfind('\\') {
        &first[pos + 1..]
    } else {
        first
    };

    // Reconstruct the command with base name
    let reconstructed = if parts.len() > 1 {
        format!("{} {}", base_name, parts[1])
    } else {
        base_name.to_string()
    };

    // Unwrap common wrappers (env, nohup, nice) and recurse once
    let wrappers = ["env", "nohup", "nice"];
    if wrappers.contains(&base_name) {
        if let Some(rest) = reconstructed.strip_prefix(base_name) {
            let rest = rest.trim_start();
            // Skip env flags like -u VAR or -i
            let inner = skip_env_flags(base_name, rest);
            if !inner.is_empty() {
                return extract_base_command(inner);
            }
        }
    }

    reconstructed
}

/// Skip common flags for wrapper commands to find the actual command.
fn skip_env_flags<'a>(wrapper: &str, rest: &'a str) -> &'a str {
    if wrapper != "env" {
        return rest;
    }
    let mut remainder = rest;
    loop {
        remainder = remainder.trim_start();
        if remainder.starts_with("-u") || remainder.starts_with("-i") || remainder.starts_with("--") {
            // Skip this flag and its argument
            if let Some(pos) = remainder.find(char::is_whitespace) {
                remainder = &remainder[pos..];
                // -u takes an argument
                if remainder.trim_start().starts_with('-') {
                    continue;
                }
            } else {
                return "";
            }
        } else {
            break;
        }
    }
    remainder
}

async fn execute_shell(command: &str, cwd: &str) -> String {
    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let arg = if cfg!(target_os = "windows") { "/C" } else { "-c" };

    let child = match tokio::process::Command::new(shell)
        .arg(arg)
        .arg(command)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return format!("Failed to execute command: {}", e),
    };

    // 30s timeout — wait_with_output consumes child, so timeout handles kill implicitly
    // (dropping child kills process on Unix)
    match tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                if stdout.is_empty() && stderr.is_empty() {
                    "(command completed successfully, no output)".to_string()
                } else if stderr.is_empty() {
                    stdout.to_string()
                } else {
                    format!("{}\n[stderr]: {}", stdout, stderr)
                }
            } else {
                format!("Command failed (exit code: {:?}):\n{}{}",
                    output.status.code(), stdout, stderr)
            }
        }
        Ok(Err(e)) => format!("Command execution error: {}", e),
        Err(_) => {
            // Timeout — dropping the future drops the child, which kills it on Unix.
            "Command timed out (30s limit). Process terminated.".to_string()
        }
    }
}

#[async_trait]
impl ToolPlugin for RunShellTool {
    fn name(&self) -> &str { "run_shell" }
    fn description(&self) -> &str {
        "Execute a shell command in the workspace directory. Safe read-only commands (ls, cat, git status, etc.) run automatically. Potentially destructive commands require user approval. Dangerous commands (rm, sudo, git push) are blocked."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let command = input["command"].as_str().unwrap_or("");
        if command.is_empty() {
            return Ok(ToolOutput::error("No command provided.".to_string()));
        }

        let cwd = &ctx.workspace_path;
        if cwd.is_empty() {
            return Ok(ToolOutput::error("No workspace directory is open. Open a directory first before running shell commands.".to_string()));
        }
        let tier = classify_command(command);

        match tier {
            CommandTier::Blocked => {
                Ok(ToolOutput::error(format!(
                    "Command blocked for security: '{}'. This command is not allowed.",
                    command
                )))
            }
            CommandTier::AutoAllow => {
                let result = execute_shell(command, cwd).await;
                Ok(ToolOutput::success(result))
            }
            CommandTier::NeedConfirm => {
                // Request user confirmation via frontend dialog
                let app = &ctx.app_handle;

                // Create a oneshot channel for the response
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

                // Store the sender in managed state
                {
                    let state = app.state::<crate::ai::ShellConfirmState>();
                    let mut sender = state.sender.lock().map_err(|e|
                        ToolError::ExecutionFailed(format!("Lock error: {}", e))
                    )?;
                    *sender = Some(tx);
                }

                // Emit event to frontend
                let _ = app.emit("shell-confirm", serde_json::json!({
                    "command": command,
                }));

                // Wait for response with 60s timeout
                match tokio::time::timeout(Duration::from_secs(60), rx).await {
                    Ok(Ok(true)) => {
                        let result = execute_shell(command, cwd).await;
                        Ok(ToolOutput::success(result))
                    }
                    Ok(Ok(false)) => {
                        Ok(ToolOutput::error("Command denied by user.".to_string()))
                    }
                    Ok(Err(_)) => {
                        Ok(ToolOutput::error("Confirmation channel closed unexpectedly.".to_string()))
                    }
                    Err(_) => {
                        // Timeout — clean up sender
                        let state = app.state::<crate::ai::ShellConfirmState>();
                        if let Ok(mut sender) = state.sender.lock() {
                            *sender = None;
                        }
                        Ok(ToolOutput::error("Command confirmation timed out (60s). Command not executed.".to_string()))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_base_command tests ---

    #[test]
    fn test_extract_base_command_simple() {
        assert_eq!(extract_base_command("ls"), "ls");
    }

    #[test]
    fn test_extract_base_command_strips_bin_path() {
        assert_eq!(extract_base_command("/bin/rm"), "rm");
    }

    #[test]
    fn test_extract_base_command_strips_usr_bin_path() {
        assert_eq!(extract_base_command("/usr/bin/git"), "git");
    }

    #[test]
    fn test_extract_base_command_with_args() {
        assert_eq!(extract_base_command("rm -rf /"), "rm -rf /");
    }

    #[test]
    fn test_extract_base_command_path_with_args() {
        assert_eq!(extract_base_command("/bin/rm -rf /"), "rm -rf /");
    }

    // --- split_command_operators tests ---

    #[test]
    fn test_split_operators_and() {
        let parts = split_command_operators("ls && rm");
        assert_eq!(parts, vec!["ls ", " rm"]);
    }

    #[test]
    fn test_split_operators_single_command() {
        let parts = split_command_operators("echo hello");
        assert_eq!(parts, vec!["echo hello"]);
    }

    #[test]
    fn test_split_operators_pipe() {
        let parts = split_command_operators("cat file | grep foo");
        assert_eq!(parts, vec!["cat file ", " grep foo"]);
    }

    #[test]
    fn test_split_operators_semicolon() {
        let parts = split_command_operators("echo a; echo b");
        assert_eq!(parts, vec!["echo a", " echo b"]);
    }

    #[test]
    fn test_split_operators_or() {
        let parts = split_command_operators("cmd1 || cmd2");
        assert_eq!(parts, vec!["cmd1 ", " cmd2"]);
    }

    // --- classify_command tests ---

    #[test]
    fn test_classify_ls_auto_allow() {
        assert_eq!(classify_command("ls"), CommandTier::AutoAllow);
    }

    #[test]
    fn test_classify_cat_auto_allow() {
        assert_eq!(classify_command("cat file.txt"), CommandTier::AutoAllow);
    }

    #[test]
    fn test_classify_git_status_auto_allow() {
        assert_eq!(classify_command("git status"), CommandTier::AutoAllow);
    }

    #[test]
    fn test_classify_git_diff_auto_allow() {
        assert_eq!(classify_command("git diff"), CommandTier::AutoAllow);
    }

    #[test]
    fn test_classify_mkdir_need_confirm() {
        assert_eq!(classify_command("mkdir foo"), CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_npm_install_need_confirm() {
        assert_eq!(classify_command("npm install"), CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_git_commit_need_confirm() {
        assert_eq!(classify_command("git commit -m 'msg'"), CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_rm_blocked() {
        assert_eq!(classify_command("rm file"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_rm_rf_blocked() {
        assert_eq!(classify_command("rm -rf /"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_sudo_blocked() {
        assert_eq!(classify_command("sudo anything"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_git_push_blocked() {
        assert_eq!(classify_command("git push"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_subshell_dollar_paren() {
        // $() subshell should be at least NeedConfirm
        let tier = classify_command("echo $(whoami)");
        assert!(tier >= CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_subshell_backtick() {
        let tier = classify_command("echo `whoami`");
        assert!(tier >= CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_pipe_takes_highest_risk() {
        // "ls | rm" — ls is AutoAllow, rm is Blocked, result should be Blocked
        assert_eq!(classify_command("ls | rm"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_chained_and_takes_highest_risk() {
        // "ls && mkdir foo" — ls is AutoAllow, mkdir is NeedConfirm
        assert_eq!(classify_command("ls && mkdir foo"), CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_unknown_command_need_confirm() {
        // Unknown commands default to NeedConfirm
        assert_eq!(classify_command("some_unknown_tool"), CommandTier::NeedConfirm);
    }

    #[test]
    fn test_classify_path_prefixed_blocked() {
        // /bin/rm should still be blocked
        assert_eq!(classify_command("/bin/rm -rf /"), CommandTier::Blocked);
    }

    #[test]
    fn test_classify_env_wrapper_unwrap() {
        // "env ls" should unwrap to "ls" -> AutoAllow
        assert_eq!(classify_command("env ls"), CommandTier::AutoAllow);
    }
}
