use async_trait::async_trait;
use reqwest::Client;
use super::{SearchEngine, SearchResult};

pub struct TavilyEngine;

#[async_trait]
impl SearchEngine for TavilyEngine {
    fn name(&self) -> &str { "Tavily" }
    fn needs_api_key(&self) -> bool { true }

    async fn search(&self, query: &str, api_key: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
        let client = Client::new();
        let body = serde_json::json!({
            "api_key": api_key,
            "query": query,
            "max_results": max_results,
        });
        let resp = client
            .post("https://api.tavily.com/search")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Tavily search request failed: {}", e))?;
        let json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Failed to parse Tavily results: {}", e))?;
        let arr = json["results"].as_array()
            .ok_or_else(|| "Tavily returned no results".to_string())?;

        let results = arr.iter().map(|r| SearchResult {
            title: r["title"].as_str().unwrap_or("").to_string(),
            url: r["url"].as_str().unwrap_or("").to_string(),
            snippet: r["content"].as_str().unwrap_or("").to_string(),
        }).collect();

        Ok(results)
    }
}
