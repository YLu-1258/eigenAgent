// src-tauri/src/tools/types.rs

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Search,
    Web,
    FileSystem,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: ToolCategory,
    pub requires_confirmation: bool,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRequest {
    pub tool_id: String,
    pub call_id: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub call_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolCallResult {
    pub fn success(call_id: String, output: String) -> Self {
        Self {
            call_id,
            success: true,
            output,
            error: None,
        }
    }

    pub fn error(call_id: String, error: String) -> Self {
        Self {
            call_id,
            success: false,
            output: String::new(),
            error: Some(error),
        }
    }
}
