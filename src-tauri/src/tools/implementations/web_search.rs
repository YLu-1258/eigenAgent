// src-tauri/src/tools/implementations/web_search.rs

use serde::Deserialize;

use crate::tools::types::{ToolCallRequest, ToolCallResult};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct DuckDuckGoResponse {
    #[serde(default)]
    abstract_text: String,
    #[serde(default)]
    abstract_source: String,
    #[serde(default)]
    abstract_url: String,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    related_topics: Vec<RelatedTopic>,
    #[serde(default)]
    results: Vec<DdgResult>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct RelatedTopic {
    #[serde(default)]
    text: String,
    #[serde(default)]
    first_url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct DdgResult {
    #[serde(default)]
    text: String,
    #[serde(default)]
    first_url: String,
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

    // Use DuckDuckGo Instant Answer API
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        urlencoding::encode(query)
    );

    let client = reqwest::Client::new();

    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to perform web search: {}", e),
            )
        }
    };

    let data: DuckDuckGoResponse = match response.json().await {
        Ok(data) => data,
        Err(e) => {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Failed to parse search response: {}", e),
            )
        }
    };

    let mut output = String::new();

    // Add main result if available
    if !data.abstract_text.is_empty() {
        output.push_str(&format!("# {}\n\n", data.heading));
        output.push_str(&format!("{}\n\n", data.abstract_text));
        if !data.abstract_url.is_empty() {
            output.push_str(&format!("Source: {} ({})\n\n", data.abstract_source, data.abstract_url));
        }
    }

    // Add direct results
    if !data.results.is_empty() {
        output.push_str("## Results:\n");
        for result in data.results.iter().take(5) {
            if !result.text.is_empty() {
                output.push_str(&format!("- {}\n", result.text));
                if !result.first_url.is_empty() {
                    output.push_str(&format!("  URL: {}\n", result.first_url));
                }
            }
        }
        output.push('\n');
    }

    // Add related topics
    if !data.related_topics.is_empty() {
        output.push_str("## Related:\n");
        for topic in data.related_topics.iter().take(5) {
            if !topic.text.is_empty() {
                output.push_str(&format!("- {}\n", topic.text));
            }
        }
    }

    if output.is_empty() {
        output = format!("No instant answer available for '{}'. Try a more specific query or use Wikipedia for detailed information.", query);
    }

    ToolCallResult::success(request.call_id.clone(), output)
}
