use std::process::Command;

/// Default timeout for local git operations (10 seconds)
const GIT_LOCAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Timeout for network git operations like push/pull (60 seconds)
const GIT_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn run_git(cwd: &str, args: &[&str]) -> Result<String, String> {
    run_git_with_timeout(cwd, args, GIT_LOCAL_TIMEOUT)
}

fn run_git_network(cwd: &str, args: &[&str]) -> Result<String, String> {
    run_git_with_timeout(cwd, args, GIT_NETWORK_TIMEOUT)
}

fn run_git_with_timeout(cwd: &str, args: &[&str], timeout: std::time::Duration) -> Result<String, String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Git not installed, please install git first".to_string()
            } else {
                format!("Failed to execute git: {}", e)
            }
        })?;

    // Wait with timeout using a thread to avoid blocking the async runtime
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited — read output
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_end(&mut stderr);
                }
                if status.success() {
                    return Ok(String::from_utf8_lossy(&stdout).to_string());
                } else {
                    let err = String::from_utf8_lossy(&stderr).to_string();
                    return Err(err.trim().to_string());
                }
            }
            Ok(None) => {
                // Still running — check timeout
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Git operation timed out after {}s", timeout.as_secs()));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(format!("Failed to wait for git: {}", e)),
        }
    }
}

#[derive(serde::Serialize)]
pub struct GitFileStatus {
    path: String,
    status: String, // "M", "A", "D", "?", "R"
    staged: bool,
}

#[derive(serde::Serialize)]
pub struct GitStatusResult {
    is_repo: bool,
    branch: String,
    files: Vec<GitFileStatus>,
}

#[derive(serde::Serialize)]
pub struct GitLogEntry {
    hash: String,
    message: String,
    author: String,
    date: String,
}

#[derive(serde::Serialize)]
pub struct GitRemoteInfo {
    name: String,
    url: String,
}

#[tauri::command]
pub fn git_status(cwd: String) -> Result<GitStatusResult, String> {
    // Check if it's a git repo (with timeout protection)
    match run_git(&cwd, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(_) => {}
        Err(_) => return Ok(GitStatusResult { is_repo: false, branch: String::new(), files: vec![] }),
    }

    let branch = run_git(&cwd, &["branch", "--show-current"])
        .unwrap_or_default().trim().to_string();

    let status_output = run_git(&cwd, &["status", "--porcelain=v1"])?;
    let mut files = Vec::new();
    for line in status_output.lines() {
        if line.len() < 4 { continue; }
        let index_status = line.chars().nth(0).unwrap_or(' ');
        let work_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();

        if index_status != ' ' && index_status != '?' {
            files.push(GitFileStatus {
                path: path.clone(),
                status: index_status.to_string(),
                staged: true,
            });
        }
        if work_status != ' ' {
            let st = if work_status == '?' { "?" } else { &work_status.to_string() };
            files.push(GitFileStatus {
                path,
                status: st.to_string(),
                staged: false,
            });
        }
    }

    Ok(GitStatusResult { is_repo: true, branch, files })
}

#[tauri::command]
pub fn git_init(cwd: String) -> Result<String, String> {
    run_git(&cwd, &["init"])
}

#[tauri::command]
pub fn git_stage(cwd: String, files: Vec<String>) -> Result<(), String> {
    let mut args = vec!["add", "--"];
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(file_refs);
    run_git(&cwd, &args)?;
    Ok(())
}

#[tauri::command]
pub fn git_unstage(cwd: String, files: Vec<String>) -> Result<(), String> {
    let mut args = vec!["reset", "HEAD", "--"];
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(file_refs);
    run_git(&cwd, &args)?;
    Ok(())
}

#[tauri::command]
pub fn git_commit(cwd: String, message: String) -> Result<String, String> {
    run_git(&cwd, &["commit", "-m", &message])
}

#[tauri::command]
pub fn git_push(cwd: String, remote: String) -> Result<String, String> {
    if remote.is_empty() {
        run_git_network(&cwd, &["push"])
    } else {
        run_git_network(&cwd, &["push", &remote])
    }
}

#[tauri::command]
pub fn git_pull(cwd: String, remote: String) -> Result<String, String> {
    if remote.is_empty() {
        run_git_network(&cwd, &["pull"])
    } else {
        run_git_network(&cwd, &["pull", &remote])
    }
}

#[tauri::command]
pub fn git_remote_add(cwd: String, name: String, url: String) -> Result<(), String> {
    run_git(&cwd, &["remote", "add", &name, &url])?;
    Ok(())
}

#[tauri::command]
pub fn git_remote_list(cwd: String) -> Result<Vec<GitRemoteInfo>, String> {
    let output = run_git(&cwd, &["remote", "-v"])?;
    let mut remotes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            if seen.insert(name.clone()) {
                remotes.push(GitRemoteInfo { name, url: parts[1].to_string() });
            }
        }
    }
    Ok(remotes)
}

#[tauri::command]
pub fn git_log(cwd: String, count: u32) -> Result<Vec<GitLogEntry>, String> {
    let count_str = format!("-{}", count.min(100));
    let output = run_git(&cwd, &["log", &count_str, "--pretty=format:%H|%s|%an|%ai"])?;
    let mut entries = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() == 4 {
            entries.push(GitLogEntry {
                hash: parts[0].to_string(),
                message: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
            });
        }
    }
    Ok(entries)
}

#[tauri::command]
pub fn git_config_user(cwd: String, username: String, email: String) -> Result<(), String> {
    run_git(&cwd, &["config", "user.name", &username])?;
    run_git(&cwd, &["config", "user.email", &email])?;
    Ok(())
}

#[tauri::command]
pub fn setup_ssh_key(email: String) -> Result<String, String> {
    let home = crate::app_home_dir().ok_or("Cannot get home directory")?;
    let ssh_dir = home.join(".ssh");
    std::fs::create_dir_all(&ssh_dir).map_err(|e| format!("Failed to create .ssh directory: {}", e))?;

    let key_path = ssh_dir.join("id_ed25519");
    let pub_path = key_path.with_extension("pub");
    if key_path.exists() && pub_path.exists() {
        let pub_key = std::fs::read_to_string(&pub_path)
            .map_err(|e| format!("Failed to read public key: {}", e))?;
        return Ok(pub_key);
    }

    let output = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-C", &email, "-f"])
        .arg(&key_path)
        .arg("-N")
        .arg("")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "ssh-keygen not found, please install OpenSSH".to_string()
            } else {
                format!("Failed to generate SSH key: {}", e)
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let pub_key = std::fs::read_to_string(key_path.with_extension("pub"))
        .map_err(|e| format!("Failed to read public key: {}", e))?;
    Ok(pub_key)
}
