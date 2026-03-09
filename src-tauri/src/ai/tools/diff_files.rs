use std::fs;
use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;

pub struct DiffFilesTool;

#[async_trait]
impl ToolPlugin for DiffFilesTool {
    fn name(&self) -> &str { "diff_files" }
    fn description(&self) -> &str {
        "Compare two files and show differences in unified diff format. If only path_a is provided, compares with the most recent snapshot."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path_a": { "type": "string", "description": "First file path" },
                "path_b": { "type": "string", "description": "Second file path. If omitted, compares with the latest snapshot." },
                "context_lines": { "type": "number", "description": "Lines of context around changes (default: 3)" }
            },
            "required": ["path_a"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_a = input["path_a"].as_str().unwrap_or("");
        let path_a = match sandbox_path(raw_a, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!(
                "Access denied: path '{}' is outside the current workspace.", raw_a
            ))),
        };

        let context_lines = input["context_lines"].as_u64().unwrap_or(3) as usize;

        let content_a = match fs::read_to_string(&path_a) {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read {}: {}", path_a, e))),
        };

        let (content_b, label_b) = if let Some(raw_b) = input["path_b"].as_str() {
            let path_b = match sandbox_path(raw_b, &ctx.workspace_path) {
                Some(p) => p,
                None => return Ok(ToolOutput::error(format!(
                    "Access denied: path '{}' is outside the current workspace.", raw_b
                ))),
            };
            match fs::read_to_string(&path_b) {
                Ok(c) => (c, path_b),
                Err(e) => return Ok(ToolOutput::error(format!("Failed to read {}: {}", path_b, e))),
            }
        } else {
            // Find latest snapshot for path_a
            match find_latest_snapshot(&path_a) {
                Some((content, snap_name)) => (content, format!("snapshot:{}", snap_name)),
                None => return Ok(ToolOutput::error(format!(
                    "No snapshot found for '{}'. Provide path_b to compare two files.", path_a
                ))),
            }
        };

        let lines_a: Vec<&str> = content_a.lines().collect();
        let lines_b: Vec<&str> = content_b.lines().collect();

        if lines_a == lines_b {
            return Ok(ToolOutput::success("Files are identical.".to_string()));
        }

        let diff = unified_diff(&lines_a, &lines_b, &path_a, &label_b, context_lines);
        Ok(ToolOutput::success(diff))
    }
}

/// Find the latest snapshot file for a given file path
fn find_latest_snapshot(file_path: &str) -> Option<(String, String)> {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let path_hash = &hash[..12];

    let data_dir = crate::app_data_dir();
    let snap_dir = data_dir.join("inkess").join("snapshots").join(path_hash);

    if !snap_dir.exists() {
        return None;
    }

    let mut names: Vec<String> = fs::read_dir(&snap_dir).ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".snap"))
        .collect();

    names.sort_by(|a, b| b.cmp(a)); // Latest first (timestamp prefix)
    let latest = names.first()?;
    let content = fs::read_to_string(snap_dir.join(latest)).ok()?;
    let snap_id = latest.trim_end_matches(".snap").to_string();
    Some((content, snap_id))
}

/// Compute LCS table for two line slices
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<u32>> {
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }
    dp
}

/// Produce unified diff output
fn unified_diff(a: &[&str], b: &[&str], label_a: &str, label_b: &str, context: usize) -> String {
    // For very large files, fall back to simple comparison
    if a.len() > 10000 || b.len() > 10000 {
        return simple_diff(a, b, label_a, label_b);
    }

    // Build edit script from LCS
    let dp = lcs_table(a, b);
    let mut edits: Vec<DiffLine> = Vec::new();

    let mut i = a.len();
    let mut j = b.len();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            edits.push(DiffLine::Context(i, j, a[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            edits.push(DiffLine::Added(j, b[j - 1]));
            j -= 1;
        } else {
            edits.push(DiffLine::Removed(i, a[i - 1]));
            i -= 1;
        }
    }
    edits.reverse();

    // Group changes into hunks with context
    let mut hunks: Vec<Vec<&DiffLine>> = Vec::new();
    let mut current_hunk: Vec<&DiffLine> = Vec::new();
    let mut last_change_idx: Option<usize> = None;

    for (idx, edit) in edits.iter().enumerate() {
        let is_change = !matches!(edit, DiffLine::Context(_, _, _));
        if is_change {
            if let Some(last) = last_change_idx {
                if idx - last > context * 2 {
                    // Too far from last change; finalize current hunk
                    if !current_hunk.is_empty() {
                        // Add trailing context
                        let trail_start = last + 1;
                        let trail_end = (trail_start + context).min(idx);
                        for t in trail_start..trail_end {
                            current_hunk.push(&edits[t]);
                        }
                        hunks.push(current_hunk);
                        current_hunk = Vec::new();
                    }
                    // Add leading context for new hunk
                    let lead_start = if idx >= context { idx - context } else { 0 };
                    for l in lead_start..idx {
                        current_hunk.push(&edits[l]);
                    }
                } else {
                    // Close enough, fill gap
                    for g in (last + 1)..idx {
                        current_hunk.push(&edits[g]);
                    }
                }
            } else {
                // First change - add leading context
                let lead_start = if idx >= context { idx - context } else { 0 };
                for l in lead_start..idx {
                    current_hunk.push(&edits[l]);
                }
            }
            current_hunk.push(&edits[idx]);
            last_change_idx = Some(idx);
        }
    }

    // Finalize last hunk
    if !current_hunk.is_empty() {
        if let Some(last) = last_change_idx {
            let trail_start = last + 1;
            let trail_end = (trail_start + context).min(edits.len());
            for t in trail_start..trail_end {
                current_hunk.push(&edits[t]);
            }
        }
        hunks.push(current_hunk);
    }

    if hunks.is_empty() {
        return "Files are identical.".to_string();
    }

    let mut output = format!("--- {}\n+++ {}\n", label_a, label_b);
    for hunk in &hunks {
        // Compute hunk header
        let (mut a_start, mut a_count, mut b_start, mut b_count) = (0usize, 0usize, 0usize, 0usize);
        let mut first = true;
        for line in hunk {
            match line {
                DiffLine::Context(ai, bi, _) => {
                    if first { a_start = *ai; b_start = *bi; first = false; }
                    a_count += 1;
                    b_count += 1;
                }
                DiffLine::Removed(ai, _) => {
                    if first { a_start = *ai; b_start = 0; first = false; }
                    a_count += 1;
                }
                DiffLine::Added(bi, _) => {
                    if first { a_start = 0; b_start = *bi; first = false; }
                    b_count += 1;
                }
            }
        }
        // Fix b_start if it was never set from a context or added line
        if b_start == 0 && a_start > 0 {
            // Estimate b_start from first context line in hunk if any
            for line in hunk {
                if let DiffLine::Context(_, bi, _) = line {
                    b_start = bi - b_count + 1;
                    break;
                }
                if let DiffLine::Added(bi, _) = line {
                    b_start = *bi;
                    break;
                }
            }
        }

        output.push_str(&format!("@@ -{},{} +{},{} @@\n", a_start, a_count, b_start, b_count));
        for line in hunk {
            match line {
                DiffLine::Context(_, _, text) => output.push_str(&format!(" {}\n", text)),
                DiffLine::Removed(_, text) => output.push_str(&format!("-{}\n", text)),
                DiffLine::Added(_, text) => output.push_str(&format!("+{}\n", text)),
            }
        }
    }

    output
}

enum DiffLine<'a> {
    Context(usize, usize, &'a str),  // (line_in_a, line_in_b, text)
    Removed(usize, &'a str),          // (line_in_a, text)
    Added(usize, &'a str),            // (line_in_b, text)
}

/// Simple fallback diff for very large files
fn simple_diff(a: &[&str], b: &[&str], label_a: &str, label_b: &str) -> String {
    let mut output = format!("--- {}\n+++ {}\n(simple comparison, files too large for LCS)\n\n", label_a, label_b);
    let max_len = a.len().max(b.len());
    let mut diffs = 0;
    for i in 0..max_len {
        let la = a.get(i).copied().unwrap_or("");
        let lb = b.get(i).copied().unwrap_or("");
        if la != lb {
            diffs += 1;
            if diffs <= 200 {
                output.push_str(&format!("Line {}:\n-{}\n+{}\n", i + 1, la, lb));
            }
        }
    }
    if diffs > 200 {
        output.push_str(&format!("\n... and {} more differences (showing first 200)\n", diffs - 200));
    }
    if a.len() != b.len() {
        output.push_str(&format!("\nLine count: {} vs {}\n", a.len(), b.len()));
    }
    output.push_str(&format!("\nTotal differences: {}\n", diffs));
    output
}
