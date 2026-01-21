// src-tauri/src/state.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::AtomicBool,
    Arc, Mutex,
};

use tauri_plugin_shell::process::CommandChild;

use crate::settings::AppSettings;

pub const MAX_TOKENS: u32 = 8192;
pub const SERVER_PORT: u16 = 8080;

pub struct LlamaServerManager {
    pub process: Mutex<Option<CommandChild>>,
    pub server_url: String,
    pub is_ready: AtomicBool,
    pub is_cancelled: AtomicBool,
    pub db_path: PathBuf,
    pub models_dir: PathBuf,
    pub model_path: Mutex<PathBuf>,
    pub mmproj_path: Mutex<Option<PathBuf>>,
    pub current_model_id: Mutex<Option<String>>,
    pub active_downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub downloading_progress: Mutex<HashMap<String, f32>>,
    pub app_settings: Mutex<AppSettings>,
}
