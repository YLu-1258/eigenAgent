// src-tauri/src/tools/openai_format.rs

use serde_json::{json, Value};

use super::types::ToolDefinition;

/// Convert tool definitions to OpenAI function calling format
pub fn tools_to_openai_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.id,
                    "description": tool.description,
                    "parameters": tool.parameters
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::get_all_tools;

    #[test]
    fn test_tools_to_openai_format() {
        let tools = get_all_tools();
        let formatted = tools_to_openai_format(&tools);

        assert!(!formatted.is_empty());

        for tool_json in &formatted {
            assert_eq!(tool_json["type"], "function");
            assert!(tool_json["function"]["name"].is_string());
            assert!(tool_json["function"]["description"].is_string());
            assert!(tool_json["function"]["parameters"].is_object());
        }
    }
}
