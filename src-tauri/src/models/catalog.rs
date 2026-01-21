// src-tauri/src/models/catalog.rs

use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

use crate::types::{ModelCatalog, ModelCatalogEntry};

pub fn get_catalog_path(app: &AppHandle) -> Result<PathBuf, String> {
    use crate::models::discovery::get_models_dir;
    Ok(get_models_dir(app)?.join("model-catalog.json"))
}

pub fn load_or_create_catalog(app: &AppHandle) -> Result<ModelCatalog, String> {
    let catalog_path = get_catalog_path(app)?;

    if catalog_path.exists() {
        let content = std::fs::read_to_string(&catalog_path).map_err(|e| e.to_string())?;
        let catalog: ModelCatalog = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        return Ok(catalog);
    }

    // Try to load from bundled resources
    let bundled_catalog = app
        .path()
        .resource_dir()
        .ok()
        .map(|p| p.join("resources/model-catalog.json"));

    if let Some(ref bundled_path) = bundled_catalog {
        if bundled_path.exists() {
            let content = std::fs::read_to_string(bundled_path).map_err(|e| e.to_string())?;
            // Copy to user directory
            std::fs::write(&catalog_path, &content).map_err(|e| e.to_string())?;
            let catalog: ModelCatalog = serde_json::from_str(&content).map_err(|e| e.to_string())?;
            println!("[catalog] Copied bundled catalog to {}", catalog_path.display());
            return Ok(catalog);
        }
    }

    // Create default catalog
    let default_catalog = ModelCatalog {
        version: 1,
        models: vec![],
    };
    let content = serde_json::to_string_pretty(&default_catalog).map_err(|e| e.to_string())?;
    std::fs::write(&catalog_path, content).map_err(|e| e.to_string())?;
    println!("[catalog] Created default catalog at {}", catalog_path.display());
    Ok(default_catalog)
}

pub fn get_model_dir(models_dir: &Path, model_id: &str) -> PathBuf {
    models_dir.join(model_id)
}

pub fn is_model_downloaded(models_dir: &Path, entry: &ModelCatalogEntry) -> bool {
    let model_dir = get_model_dir(models_dir, &entry.id);
    let model_path = model_dir.join(&entry.files.model.filename);

    if !model_path.exists() {
        return false;
    }

    // Check mmproj if required
    if let Some(ref mmproj) = entry.files.mmproj {
        let mmproj_path = model_dir.join(&mmproj.filename);
        if !mmproj_path.exists() {
            return false;
        }
    }

    true
}

pub fn get_model_paths(models_dir: &Path, entry: &ModelCatalogEntry) -> Option<(PathBuf, Option<PathBuf>)> {
    let model_dir = get_model_dir(models_dir, &entry.id);
    let model_path = model_dir.join(&entry.files.model.filename);

    if !model_path.exists() {
        return None;
    }

    let mmproj_path = entry.files.mmproj.as_ref().map(|mmproj| {
        model_dir.join(&mmproj.filename)
    });

    // Check mmproj exists if required
    if let Some(ref path) = mmproj_path {
        if !path.exists() {
            return None;
        }
    }

    Some((model_path, mmproj_path))
}
