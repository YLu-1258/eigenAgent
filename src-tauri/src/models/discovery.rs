// src-tauri/src/models/discovery.rs

use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

/// Scans a directory for .gguf model files.
/// Returns (main_model, optional_mmproj) if found.
pub fn scan_models_dir(models_dir: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    if !models_dir.exists() {
        return None;
    }

    let entries = match std::fs::read_dir(models_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    let mut main_model: Option<PathBuf> = None;
    let mut mmproj: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "gguf" {
                    let filename = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();

                    if filename.contains("mmproj") {
                        mmproj = Some(path);
                    } else if main_model.is_none() {
                        main_model = Some(path);
                    }
                }
            }
        }
    }

    main_model.map(|m| (m, mmproj))
}

pub fn find_model_files(app: &AppHandle) -> Result<(PathBuf, Option<PathBuf>), String> {
    // 1. Check app data directory first (production location)
    let app_data_models = app
        .path()
        .app_data_dir()
        .ok()
        .map(|p| p.join("models"));

    if let Some(ref dir) = app_data_models {
        // Create the directory if it doesn't exist (so users know where to put models)
        let _ = std::fs::create_dir_all(dir);

        if let Some(result) = scan_models_dir(dir) {
            println!("[model] Found models in app data: {}", dir.display());
            return Ok(result);
        }
    }

    // 2. Fall back to development models folder (relative to project root)
    let dev_models = app
        .path()
        .resource_dir()
        .ok()
        .map(|p| p.join("../../../models"));

    if let Some(ref dir) = dev_models {
        if let Some(result) = scan_models_dir(dir) {
            println!("[model] Found models in dev folder: {}", dir.display());
            return Ok(result);
        }
    }

    // No models found - provide helpful error message
    let app_data_path = app_data_models
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/eigenAgent/models".to_string());

    Err(format!(
        "No .gguf model files found.\n\n\
        Please place your model files in one of these locations:\n\
        • {} (recommended for production)\n\
        • ./models/ (development only)\n\n\
        The model file should have a .gguf extension.",
        app_data_path
    ))
}

pub fn get_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Detect legacy models in flat structure (for migration)
pub fn detect_legacy_model(models_dir: &Path) -> Option<String> {
    if let Some((model_path, _)) = scan_models_dir(models_dir) {
        // Check if this is a flat structure (not in a subdirectory)
        if model_path.parent() == Some(models_dir) {
            return Some("legacy".to_string());
        }
    }
    None
}
