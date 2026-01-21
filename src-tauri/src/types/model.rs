// src-tauri/src/types/model.rs

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub thinking: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelFile {
    pub filename: String,
    pub url: String,
    pub size_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelFiles {
    pub model: ModelFile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mmproj: Option<ModelFile>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_label: String,
    pub capabilities: ModelCapabilities,
    pub files: ModelFiles,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelCatalog {
    pub version: u32,
    pub models: Vec<ModelCatalogEntry>,
}

#[derive(Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_label: String,
    pub capabilities: ModelCapabilities,
    pub download_status: String, // "not_downloaded" | "downloading" | "downloaded"
    pub download_percent: Option<f32>,
    pub is_current: bool,
}

#[derive(Clone, Serialize)]
pub struct DownloadProgressPayload {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub speed_bps: u64,
}

#[derive(Clone, Serialize)]
pub struct ModelSwitchPayload {
    pub model_id: String,
    pub status: String, // "stopping" | "starting" | "ready" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct SwitchModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    pub model_id: String,
}

#[derive(Deserialize)]
pub struct DownloadModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    pub model_id: String,
}

#[derive(Deserialize)]
pub struct CancelDownloadArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    pub model_id: String,
}

#[derive(Deserialize)]
pub struct DeleteModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    pub model_id: String,
}
