pub mod duckduckgo;
pub mod tavily;
pub mod brave;
pub mod serpapi;

use async_trait::async_trait;

pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait]
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &str;
    fn needs_api_key(&self) -> bool;
    async fn search(&self, query: &str, api_key: &str, max_results: usize) -> Result<Vec<SearchResult>, String>;
}

/// Format search results as text for LLM consumption
pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No search results found".to_string();
    }
    let mut output = format!("Search results for \"{}\":\n\n", query);
    for (i, r) in results.iter().enumerate() {
        output.push_str(&format!("{}. {} - {}\n", i + 1, r.title, r.url));
        if !r.snippet.is_empty() {
            output.push_str(&format!("   {}\n", r.snippet));
        }
        output.push('\n');
    }
    output
}

/// Get search engine by provider name
pub fn get_engine(provider: &str) -> Box<dyn SearchEngine> {
    match provider {
        "tavily" => Box::new(tavily::TavilyEngine),
        "brave" => Box::new(brave::BraveEngine),
        "serpapi" => Box::new(serpapi::SerpApiEngine),
        _ => Box::new(duckduckgo::DuckDuckGoEngine),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_results_with_results() {
        let results = vec![
            SearchResult {
                title: "First Result".to_string(),
                url: "https://example.com/1".to_string(),
                snippet: "This is the first snippet".to_string(),
            },
            SearchResult {
                title: "Second Result".to_string(),
                url: "https://example.com/2".to_string(),
                snippet: "This is the second snippet".to_string(),
            },
        ];

        let output = format_results("test query", &results);

        assert!(output.contains("Search results for \"test query\""));
        assert!(output.contains("1. First Result - https://example.com/1"));
        assert!(output.contains("This is the first snippet"));
        assert!(output.contains("2. Second Result - https://example.com/2"));
        assert!(output.contains("This is the second snippet"));
    }

    #[test]
    fn test_format_results_empty() {
        let results: Vec<SearchResult> = vec![];
        let output = format_results("empty query", &results);
        assert_eq!(output, "No search results found");
    }

    #[test]
    fn test_format_results_with_empty_snippet() {
        let results = vec![
            SearchResult {
                title: "No Snippet".to_string(),
                url: "https://example.com/no-snippet".to_string(),
                snippet: "".to_string(),
            },
        ];

        let output = format_results("query", &results);
        assert!(output.contains("1. No Snippet - https://example.com/no-snippet"));
        // Should not have extra indented line for empty snippet
        assert!(!output.contains("   \n"));
    }

    #[test]
    fn test_get_engine_tavily() {
        let engine = get_engine("tavily");
        assert_eq!(engine.name(), "Tavily");
    }

    #[test]
    fn test_get_engine_brave() {
        let engine = get_engine("brave");
        assert_eq!(engine.name(), "Brave Search");
    }

    #[test]
    fn test_get_engine_serpapi() {
        let engine = get_engine("serpapi");
        assert_eq!(engine.name(), "SerpAPI");
    }

    #[test]
    fn test_get_engine_default_duckduckgo() {
        let engine = get_engine("unknown");
        assert_eq!(engine.name(), "DuckDuckGo");

        let engine2 = get_engine("");
        assert_eq!(engine2.name(), "DuckDuckGo");
    }
}
