// src-tauri/src/commands/tools.rs

use tauri::State;

use crate::settings::save_settings;
use crate::state::LlamaServerManager;
use crate::tools::{get_all_tools, get_tool_by_id, ToolDefinition};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolWithStatus {
    #[serde(flatten)]
    pub definition: ToolDefinition,
    pub enabled: bool,
}

#[tauri::command]
pub fn cmd_list_tools(state: State<'_, LlamaServerManager>) -> Result<Vec<ToolWithStatus>, String> {
    let settings = state
        .app_settings
        .lock()
        .map_err(|e| format!("Failed to lock settings: {}", e))?;

    let enabled_tools = &settings.tools.enabled_tools;

    let tools: Vec<ToolWithStatus> = get_all_tools()
        .into_iter()
        .map(|def| {
            let enabled = enabled_tools.contains(&def.id);
            ToolWithStatus {
                definition: def,
                enabled,
            }
        })
        .collect();

    Ok(tools)
}

#[tauri::command]
pub fn cmd_toggle_tool(
    tool_id: String,
    enabled: bool,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    // Verify tool exists
    if get_tool_by_id(&tool_id).is_none() {
        return Err(format!("Unknown tool: {}", tool_id));
    }

    let mut settings = state
        .app_settings
        .lock()
        .map_err(|e| format!("Failed to lock settings: {}", e))?;

    if enabled {
        if !settings.tools.enabled_tools.contains(&tool_id) {
            settings.tools.enabled_tools.push(tool_id.clone());
            println!("[tools] Enabled tool: {}", tool_id);
        }
    } else {
        settings.tools.enabled_tools.retain(|id| id != &tool_id);
        println!("[tools] Disabled tool: {}", tool_id);
    }

    // Save settings to disk
    save_settings(&settings)?;

    Ok(())
}

#[tauri::command]
pub fn cmd_get_enabled_tools(state: State<'_, LlamaServerManager>) -> Result<Vec<ToolDefinition>, String> {
    let settings = state
        .app_settings
        .lock()
        .map_err(|e| format!("Failed to lock settings: {}", e))?;

    let enabled_tools: Vec<ToolDefinition> = get_all_tools()
        .into_iter()
        .filter(|tool| settings.tools.enabled_tools.contains(&tool.id))
        .collect();

    Ok(enabled_tools)
}
