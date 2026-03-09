use async_trait::async_trait;
use reqwest::Client;
use crate::ai::gateway::urlencoding;
use super::{SearchEngine, SearchResult};

pub struct DuckDuckGoEngine;

#[async_trait]
impl SearchEngine for DuckDuckGoEngine {
    fn name(&self) -> &str { "DuckDuckGo" }
    fn needs_api_key(&self) -> bool { false }

    async fn search(&self, query: &str, _api_key: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
        let client = Client::new();
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding(query));
        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; Inkess/1.0)")
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;
        let html = resp.text().await
            .map_err(|e| format!("Failed to read search results: {}", e))?;

        // Parse titles and URLs from DuckDuckGo HTML
        let mut titles_urls: Vec<(String, String)> = Vec::new();
        for part in html.split("class=\"result__a\"") {
            if titles_urls.len() >= max_results { break; }
            if let Some(href_start) = part.find("href=\"") {
                let rest = &part[href_start + 6..];
                if let Some(href_end) = rest.find('"') {
                    let href = &rest[..href_end];
                    if let Some(tag_end) = rest.find('>') {
                        let after_tag = &rest[tag_end + 1..];
                        if let Some(close) = after_tag.find("</a>") {
                            let title = html_unescape(&after_tag[..close]);
                            if !title.is_empty() && !href.is_empty() {
                                titles_urls.push((title, href.to_string()));
                            }
                        }
                    }
                }
            }
        }

        // Parse snippets
        let mut snippets: Vec<String> = Vec::new();
        for part in html.split("class=\"result__snippet\"") {
            if snippets.len() >= max_results { break; }
            if let Some(tag_end) = part.find('>') {
                let after = &part[tag_end + 1..];
                if let Some(close) = after.find("</a>") {
                    let snippet = html_unescape(&after[..close]);
                    if !snippet.is_empty() {
                        snippets.push(snippet);
                    }
                }
            }
        }

        let results = titles_urls.into_iter().enumerate().map(|(i, (title, url))| {
            SearchResult {
                title,
                url,
                snippet: snippets.get(i).cloned().unwrap_or_default(),
            }
        }).collect();

        Ok(results)
    }
}

fn html_unescape(s: &str) -> String {
    s.replace("<b>", "").replace("</b>", "")
        .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&#x27;", "'").replace("&quot;", "\"")
        .trim().to_string()
}
