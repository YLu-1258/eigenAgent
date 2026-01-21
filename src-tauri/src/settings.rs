// src-tauri/src/settings.rs

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are Eigen, a helpful AI assistant.

Rules:
- Use Markdown for formatting.
- Use LaTeX ($...$ / $$...$$) for math.
- If you don't know, say "I don't know"."#;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: String,           // "dark" | "light" | "system"
    pub accent_color: String,    // hex color like "#3b82f6"
    pub font_size: String,       // "small" | "medium" | "large"
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            accent_color: "#3b82f6".to_string(),
            font_size: "medium".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSettings {
    pub model_id: Option<String>,
    pub system_prompt: String,
}

impl Default for DefaultSettings {
    fn default() -> Self {
        Self {
            model_id: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorSettings {
    pub send_on_enter: bool,
    pub streaming_enabled: bool,
    pub context_length: u32,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            send_on_enter: true,
            streaming_enabled: true,
            context_length: 8192,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub version: u32,
    pub appearance: AppearanceSettings,
    pub defaults: DefaultSettings,
    pub behavior: BehaviorSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: AppearanceSettings::default(),
            defaults: DefaultSettings::default(),
            behavior: BehaviorSettings::default(),
        }
    }
}

/// Get the path to the settings file (~/.config/eigenAgent/settings.json)
pub fn get_settings_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?;

    let app_config_dir = config_dir.join("eigenAgent");

    // Create directory if it doesn't exist
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    Ok(app_config_dir.join("settings.json"))
}

/// Load settings from disk, creating default if not exists
pub fn load_settings() -> Result<AppSettings, String> {
    let path = get_settings_path()?;

    if !path.exists() {
        // Create default settings
        let default_settings = AppSettings::default();
        save_settings(&default_settings)?;
        println!("[settings] Created default settings at {}", path.display());
        return Ok(default_settings);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: AppSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    println!("[settings] Loaded settings from {}", path.display());
    Ok(settings)
}

/// Save settings to disk
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path()?;

    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    println!("[settings] Saved settings to {}", path.display());
    Ok(())
}

/// Get default settings (for reset functionality)
pub fn get_default_settings() -> AppSettings {
    AppSettings::default()
}

/// Get the default system prompt
pub fn get_default_system_prompt() -> &'static str {
    DEFAULT_SYSTEM_PROMPT
}
