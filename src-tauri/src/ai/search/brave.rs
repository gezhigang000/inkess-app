use async_trait::async_trait;
use reqwest::Client;
use crate::ai::gateway::urlencoding;
use super::{SearchEngine, SearchResult};

pub struct BraveEngine;

#[async_trait]
impl SearchEngine for BraveEngine {
    fn name(&self) -> &str { "Brave Search" }
    fn needs_api_key(&self) -> bool { true }

    async fn search(&self, query: &str, api_key: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
        let client = Client::new();
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding(query),
            max_results
        );
        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", api_key)
            .send()
            .await
            .map_err(|e| format!("Brave search request failed: {}", e))?;
        let json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Failed to parse Brave results: {}", e))?;
        let arr = json["web"]["results"].as_array()
            .ok_or_else(|| "Brave returned no results".to_string())?;

        let results = arr.iter().map(|r| SearchResult {
            title: r["title"].as_str().unwrap_or("").to_string(),
            url: r["url"].as_str().unwrap_or("").to_string(),
            snippet: r["description"].as_str().unwrap_or("").to_string(),
        }).collect();

        Ok(results)
    }
}
