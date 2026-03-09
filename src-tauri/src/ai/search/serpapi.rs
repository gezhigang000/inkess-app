use async_trait::async_trait;
use reqwest::Client;
use crate::ai::gateway::urlencoding;
use super::{SearchEngine, SearchResult};

pub struct SerpApiEngine;

#[async_trait]
impl SearchEngine for SerpApiEngine {
    fn name(&self) -> &str { "SerpAPI" }
    fn needs_api_key(&self) -> bool { true }

    async fn search(&self, query: &str, api_key: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
        let client = Client::new();
        let url = format!(
            "https://serpapi.com/search.json?q={}&api_key={}&num={}",
            urlencoding(query),
            urlencoding(api_key),
            max_results
        );
        let resp = client.get(&url).send().await
            .map_err(|e| format!("SerpAPI search request failed: {}", e))?;
        let json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Failed to parse SerpAPI results: {}", e))?;
        let arr = json["organic_results"].as_array()
            .ok_or_else(|| "SerpAPI returned no results".to_string())?;

        let results = arr.iter().map(|r| SearchResult {
            title: r["title"].as_str().unwrap_or("").to_string(),
            url: r["link"].as_str().unwrap_or("").to_string(),
            snippet: r["snippet"].as_str().unwrap_or("").to_string(),
        }).collect();

        Ok(results)
    }
}
