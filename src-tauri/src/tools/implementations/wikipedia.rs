// src-tauri/src/tools/implementations/wikipedia.rs

use serde::Deserialize;

use crate::tools::types::{ToolCallRequest, ToolCallResult};

#[derive(Deserialize)]
struct WikipediaSearchResponse {
    #[serde(default)]
    query: Option<WikipediaQuery>,
}

#[derive(Deserialize)]
struct WikipediaQuery {
    #[serde(default)]
    search: Vec<WikipediaSearchResult>,
}

#[derive(Deserialize)]
struct WikipediaSearchResult {
    title: String,
    snippet: String,
    pageid: u64,
}

#[derive(Deserialize)]
struct WikipediaContentResponse {
    #[serde(default)]
    query: Option<WikipediaContentQuery>,
}

#[derive(Deserialize)]
struct WikipediaContentQuery {
    #[serde(default)]
    pages: std::collections::HashMap<String, WikipediaPage>,
}

#[derive(Deserialize)]
struct WikipediaPage {
    title: String,
    #[serde(default)]
    extract: Option<String>,
}

pub async fn execute(request: &ToolCallRequest) -> ToolCallResult {
    let query = match request.arguments.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => {
            return ToolCallResult::error(
                request.call_id.clone(),
                "Missing required parameter: query".to_string(),
            )
        }
    };

    // First, search for articles
    let search_url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=3",
        urlencoding::encode(query)
    );

    let client = reqwest::Client::new();

    let search_response = match client.get(&search_url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to search Wikipedia: {}", e),
            )
        }
    };

    let search_data: WikipediaSearchResponse = match search_response.json().await {
        Ok(data) => data,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to parse Wikipedia search response: {}", e),
            )
        }
    };

    let results = match search_data.query {
        Some(q) => q.search,
        None => {
            return ToolCallResult::success(
                request.call_id.clone(),
                format!("No Wikipedia articles found for '{}'", query),
            )
        }
    };

    if results.is_empty() {
        return ToolCallResult::success(
            request.call_id.clone(),
            format!("No Wikipedia articles found for '{}'", query),
        );
    }

    // Get the content of the first result
    let page_title = &results[0].title;
    let content_url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&titles={}&prop=extracts&exintro=true&explaintext=true&format=json",
        urlencoding::encode(page_title)
    );

    let content_response = match client.get(&content_url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to fetch Wikipedia article: {}", e),
            )
        }
    };

    let content_data: WikipediaContentResponse = match content_response.json().await {
        Ok(data) => data,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to parse Wikipedia content response: {}", e),
            )
        }
    };

    let extract = content_data
        .query
        .and_then(|q| {
            q.pages
                .values()
                .next()
                .and_then(|p| p.extract.clone())
        })
        .unwrap_or_else(|| "No content available".to_string());

    // Build output with search results and main article content
    let mut output = format!("# {}\n\n{}\n\n", page_title, extract);

    if results.len() > 1 {
        output.push_str("## Related articles:\n");
        for result in results.iter().skip(1) {
            // Strip HTML tags from snippet
            let clean_snippet = strip_html_tags(&result.snippet);
            output.push_str(&format!("- **{}**: {}\n", result.title, clean_snippet));
        }
    }

    ToolCallResult::success(request.call_id.clone(), output)
}

fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result
}
