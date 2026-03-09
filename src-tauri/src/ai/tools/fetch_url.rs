use async_trait::async_trait;
use serde_json::Value;
use reqwest::Client;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::gateway::{extract_between, strip_tag_blocks, strip_html_tags};

pub struct FetchUrlTool;

#[async_trait]
impl ToolPlugin for FetchUrlTool {
    fn name(&self) -> &str { "fetch_url" }
    fn description(&self) -> &str { "Fetch and read the text content of a web page. Use after web_search to read full article content from a URL. Returns cleaned text (HTML tags stripped). Only http/https URLs allowed." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to fetch (http or https)" }
            },
            "required": ["url"]
        })
    }
    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let url = input["url"].as_str().unwrap_or("");
        let result = fetch_url(url).await;
        Ok(ToolOutput::success(result))
    }
}

async fn fetch_url(url: &str) -> String {
    if url.trim().is_empty() {
        return "Please provide a URL to fetch".to_string();
    }
    // Only allow http/https
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return "Only http and https URLs are allowed".to_string();
    }
    // Block localhost and private IPs (SSRF protection)
    let lower = url.to_lowercase();
    let blocked = [
        "://localhost", "://127.", "://0.0.0.0", "://0/", "://0.",
        "://10.", "://192.168.", "://169.254.",
        "://172.16.", "://172.17.", "://172.18.", "://172.19.",
        "://172.20.", "://172.21.", "://172.22.", "://172.23.",
        "://172.24.", "://172.25.", "://172.26.", "://172.27.",
        "://172.28.", "://172.29.", "://172.30.", "://172.31.",
        "://[::1]", "://[fc", "://[fd", "://[fe80",
    ];
    if blocked.iter().any(|b| lower.contains(b)) {
        return "Access to local/private addresses is not allowed".to_string();
    }

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Failed to create HTTP client: {}", e),
    };

    let resp = match client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; Inkess/1.0)")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Fetch failed: {}", e),
    };

    if !resp.status().is_success() {
        return format!("HTTP error: {}", resp.status());
    }

    // Limit response size to 2MB
    let content_length = resp.content_length().unwrap_or(0);
    if content_length > 2 * 1024 * 1024 {
        return format!("Response too large: {} bytes (max 2MB)", content_length);
    }

    let html = match resp.text().await {
        Ok(t) => {
            if t.len() > 2 * 1024 * 1024 {
                return format!("Response too large: {} bytes (max 2MB)", t.len());
            }
            t
        }
        Err(e) => return format!("Failed to read response: {}", e),
    };

    // Extract title
    let title = extract_between(&html, "<title", "</title>")
        .and_then(|t| t.find('>').map(|i| t[i + 1..].to_string()))
        .unwrap_or_default();

    // Strip script, style, nav, header, footer blocks
    let mut cleaned = html;
    for tag in &["script", "style", "nav", "header", "footer", "noscript", "svg"] {
        cleaned = strip_tag_blocks(&cleaned, tag);
    }

    // Strip all remaining HTML tags
    let text = strip_html_tags(&cleaned);

    // Clean whitespace: collapse multiple newlines/spaces
    let text = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate to 15000 chars
    let mut result = String::new();
    if !title.is_empty() {
        result.push_str(&format!("Title: {}\n\n", title.trim()));
    }
    result.push_str(&format!("URL: {}\n\n", url));

    let remaining = 15000 - result.len().min(15000);
    if text.len() > remaining {
        let mut end = remaining;
        while end > 0 && !text.is_char_boundary(end) { end -= 1; }
        if end == 0 {
            result.push_str("\n\n[Content too large to display]");
        } else {
            result.push_str(&text[..end]);
            result.push_str("\n\n[Content truncated]");
        }
    } else {
        result.push_str(&text);
    }

    result
}
