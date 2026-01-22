// src-tauri/src/tools/registry.rs

use once_cell::sync::Lazy;
use serde_json::json;

use super::types::{ToolCategory, ToolDefinition};

pub static BUILT_IN_TOOLS: Lazy<Vec<ToolDefinition>> = Lazy::new(|| {
    vec![
        ToolDefinition {
            id: "wikipedia".to_string(),
            name: "Wikipedia".to_string(),
            description: "Search and retrieve Wikipedia articles".to_string(),
            icon: "book".to_string(),
            category: ToolCategory::Search,
            requires_confirmation: false,
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find Wikipedia articles"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            id: "web_search".to_string(),
            name: "Web Search".to_string(),
            description: "Search the web using DuckDuckGo".to_string(),
            icon: "globe".to_string(),
            category: ToolCategory::Web,
            requires_confirmation: false,
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            id: "filesystem".to_string(),
            name: "File System".to_string(),
            description: "Read, write, and list files on your computer".to_string(),
            icon: "folder".to_string(),
            category: ToolCategory::FileSystem,
            requires_confirmation: true,
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["read", "write", "list"],
                        "description": "The file operation to perform"
                    },
                    "path": {
                        "type": "string",
                        "description": "The file or directory path"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write (only for write operation)"
                    }
                },
                "required": ["operation", "path"]
            }),
        },
        ToolDefinition {
            id: "shell".to_string(),
            name: "Shell".to_string(),
            description: "Execute shell commands".to_string(),
            icon: "terminal".to_string(),
            category: ToolCategory::System,
            requires_confirmation: true,
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            id: "calculator".to_string(),
            name: "Calculator".to_string(),
            description: "Evaluate mathematical expressions".to_string(),
            icon: "calculator".to_string(),
            category: ToolCategory::System,
            requires_confirmation: false,
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate (e.g., '2 + 2 * 3', 'sqrt(16)', 'sin(pi/2)')"
                    }
                },
                "required": ["expression"]
            }),
        },
    ]
});

pub fn get_all_tools() -> Vec<ToolDefinition> {
    BUILT_IN_TOOLS.clone()
}

pub fn get_tool_by_id(id: &str) -> Option<ToolDefinition> {
    BUILT_IN_TOOLS.iter().find(|t| t.id == id).cloned()
}
