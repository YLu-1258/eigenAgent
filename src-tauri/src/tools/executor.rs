// src-tauri/src/tools/executor.rs

use super::implementations::{calculator, filesystem, shell, web_search, wikipedia};
use super::types::{ToolCallRequest, ToolCallResult};

pub async fn execute_tool(request: &ToolCallRequest) -> ToolCallResult {
    match request.tool_id.as_str() {
        "wikipedia" => wikipedia::execute(request).await,
        "web_search" => web_search::execute(request).await,
        "filesystem" => filesystem::execute(request).await,
        "shell" => shell::execute(request).await,
        "calculator" => calculator::execute(request),
        _ => ToolCallResult::error(
            request.call_id.clone(),
            format!("Unknown tool: {}", request.tool_id),
        ),
    }
}
