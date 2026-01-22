// src-tauri/src/commands/model.rs

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use futures::StreamExt;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::ShellExt;
use tokio::io::AsyncWriteExt;

use crate::models::{
    detect_legacy_model, get_model_dir, get_model_paths, is_model_downloaded,
    load_or_create_catalog, scan_models_dir,
};
use crate::server::wait_for_server_ready;
use crate::state::{LlamaServerManager, SERVER_PORT};
use crate::types::{
    CancelDownloadArgs, DeleteModelArgs, DownloadModelArgs, DownloadProgressPayload,
    ModelCapabilities, ModelFile, ModelInfo, ModelSwitchPayload, SwitchModelArgs,
};

#[tauri::command]
pub fn list_models(app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<Vec<ModelInfo>, String> {
    let catalog = load_or_create_catalog(&app)?;
    let current_model_id = state.current_model_id.lock().map_err(|e| e.to_string())?;
    let downloading_progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;

    let mut models: Vec<ModelInfo> = catalog
        .models
        .iter()
        .map(|entry| {
            let download_status = if downloading_progress.contains_key(&entry.id) {
                "downloading".to_string()
            } else if is_model_downloaded(&state.models_dir, entry) {
                "downloaded".to_string()
            } else {
                "not_downloaded".to_string()
            };

            let download_percent = downloading_progress.get(&entry.id).copied();

            ModelInfo {
                id: entry.id.clone(),
                name: entry.name.clone(),
                description: entry.description.clone(),
                size_label: entry.size_label.clone(),
                capabilities: entry.capabilities.clone(),
                download_status,
                download_percent,
                is_current: current_model_id.as_ref() == Some(&entry.id),
            }
        })
        .collect();

    // Add legacy model if detected
    if let Some(_legacy_id) = detect_legacy_model(&state.models_dir) {
        // Check if we have a legacy model in flat structure
        if let Some((model_path, mmproj_path)) = scan_models_dir(&state.models_dir) {
            // Check if it's not already in a subdirectory
            if model_path.parent() == Some(&state.models_dir) {
                let model_name = model_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Legacy Model".to_string());

                models.insert(
                    0,
                    ModelInfo {
                        id: "legacy".to_string(),
                        name: model_name,
                        description: "Existing model from previous installation".to_string(),
                        size_label: "".to_string(),
                        capabilities: ModelCapabilities {
                            vision: mmproj_path.is_some(),
                            thinking: false,
                        },
                        download_status: "downloaded".to_string(),
                        download_percent: None,
                        is_current: current_model_id.as_ref() == Some(&"legacy".to_string()),
                    },
                );
            }
        }
    }

    Ok(models)
}

#[tauri::command]
pub fn get_current_model(state: State<'_, LlamaServerManager>) -> Result<Option<String>, String> {
    let current = state.current_model_id.lock().map_err(|e| e.to_string())?;
    Ok(current.clone())
}

#[tauri::command]
pub async fn switch_model(
    args: SwitchModelArgs,
    app: AppHandle,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let model_id = args.model_id;

    // Emit switching status
    let _ = app.emit(
        "model:switching",
        ModelSwitchPayload {
            model_id: model_id.clone(),
            status: "stopping".to_string(),
            error: None,
        },
    );

    // Find the model in catalog
    let catalog = load_or_create_catalog(&app)?;

    let (model_path, mmproj_path) = if model_id == "legacy" {
        // Handle legacy model
        scan_models_dir(&state.models_dir).ok_or_else(|| "Legacy model not found".to_string())?
    } else {
        let entry = catalog
            .models
            .iter()
            .find(|e| e.id == model_id)
            .ok_or_else(|| format!("Model {} not found in catalog", model_id))?;

        get_model_paths(&state.models_dir, entry)
            .ok_or_else(|| format!("Model {} is not downloaded", model_id))?
    };

    // Kill current server
    {
        let mut process_guard = state.process.lock().map_err(|e| e.to_string())?;
        if let Some(child) = process_guard.take() {
            let _ = child.kill();
            println!("[model] Killed existing server");
        }
    }

    // Mark as not ready
    state.is_ready.store(false, Ordering::SeqCst);

    // Update model paths
    {
        let mut mp = state.model_path.lock().map_err(|e| e.to_string())?;
        *mp = model_path.clone();
    }
    {
        let mut mmpp = state.mmproj_path.lock().map_err(|e| e.to_string())?;
        *mmpp = mmproj_path.clone();
    }
    {
        let mut current = state.current_model_id.lock().map_err(|e| e.to_string())?;
        *current = Some(model_id.clone());
    }

    // Emit starting status
    let _ = app.emit(
        "model:switching",
        ModelSwitchPayload {
            model_id: model_id.clone(),
            status: "starting".to_string(),
            error: None,
        },
    );

    // Start new server
    let shell = app.shell();
    let mut cmd = shell
        .sidecar("llama-server")
        .map_err(|e| e.to_string())?;

    // Get context length and max tokens from settings
    let (ctx_size, max_tokens) = {
        let settings = state.app_settings.lock().map_err(|e| e.to_string())?;
        (
            settings.behavior.context_length.to_string(),
            settings.behavior.max_tokens.to_string(),
        )
    };

    cmd = cmd
        .args(["-m", model_path.to_str().unwrap()])
        .args(["--host", "127.0.0.1"])
        .args(["--port", &SERVER_PORT.to_string()])
        .args(["--ctx-size", &ctx_size])
        .args(["--n-predict", &max_tokens]);

    if let Some(ref mmproj) = mmproj_path {
        cmd = cmd.args(["--mmproj", mmproj.to_str().unwrap()]);
    }

    match cmd.spawn() {
        Ok((mut rx, child)) => {
            // Store the child process
            {
                let mut guard = state.process.lock().map_err(|e| e.to_string())?;
                *guard = Some(child);
            }

            // Log server output in background
            let app_clone = app.clone();
            let model_id_clone = model_id.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                            println!("[llama-server] {}", String::from_utf8_lossy(&line));
                        }
                        tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                            eprintln!("[llama-server] {}", String::from_utf8_lossy(&line));
                        }
                        tauri_plugin_shell::process::CommandEvent::Error(err) => {
                            let _ = app_clone.emit(
                                "model:switching",
                                ModelSwitchPayload {
                                    model_id: model_id_clone.clone(),
                                    status: "error".to_string(),
                                    error: Some(err),
                                },
                            );
                        }
                        _ => {}
                    }
                }
            });

            // Wait for server to be ready
            let server_url = state.server_url.clone();
            match wait_for_server_ready(&server_url, 120).await {
                Ok(()) => {
                    state.is_ready.store(true, Ordering::SeqCst);
                    let _ = app.emit(
                        "model:switching",
                        ModelSwitchPayload {
                            model_id: model_id.clone(),
                            status: "ready".to_string(),
                            error: None,
                        },
                    );
                    let _ = app.emit("model:ready", ());
                    println!("[llama-server] Ready with model: {}", model_id);
                }
                Err(e) => {
                    let _ = app.emit(
                        "model:switching",
                        ModelSwitchPayload {
                            model_id: model_id.clone(),
                            status: "error".to_string(),
                            error: Some(e.clone()),
                        },
                    );
                    return Err(e);
                }
            }
        }
        Err(e) => {
            let _ = app.emit(
                "model:switching",
                ModelSwitchPayload {
                    model_id: model_id.clone(),
                    status: "error".to_string(),
                    error: Some(format!("Failed to spawn llama-server: {}", e)),
                },
            );
            return Err(format!("Failed to spawn llama-server: {}", e));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn download_model(
    args: DownloadModelArgs,
    app: AppHandle,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let model_id = args.model_id;

    // Find model in catalog
    let catalog = load_or_create_catalog(&app)?;
    let entry = catalog
        .models
        .iter()
        .find(|e| e.id == model_id)
        .ok_or_else(|| format!("Model {} not found in catalog", model_id))?
        .clone();

    // Check if already downloading
    {
        let downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
        if downloads.contains_key(&model_id) {
            return Err("Model is already being downloaded".to_string());
        }
    }

    // Create cancellation token
    let cancel_token = Arc::new(AtomicBool::new(false));
    {
        let mut downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
        downloads.insert(model_id.clone(), cancel_token.clone());
    }

    // Track progress
    {
        let mut progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;
        progress.insert(model_id.clone(), 0.0);
    }

    // Create model directory
    let model_dir = get_model_dir(&state.models_dir, &model_id);
    std::fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;

    // Calculate total bytes
    let total_bytes = entry.files.model.size_bytes
        + entry.files.mmproj.as_ref().map(|f| f.size_bytes).unwrap_or(0);

    // Download files
    let files_to_download: Vec<&ModelFile> = {
        let mut files = vec![&entry.files.model];
        if let Some(ref mmproj) = entry.files.mmproj {
            files.push(mmproj);
        }
        files
    };

    let client = reqwest::Client::new();
    let mut total_downloaded: u64 = 0;
    let start_time = Instant::now();

    for file in files_to_download {
        if cancel_token.load(Ordering::SeqCst) {
            // Cleanup on cancel
            let _ = std::fs::remove_dir_all(&model_dir);
            {
                let mut downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
                downloads.remove(&model_id);
            }
            {
                let mut progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;
                progress.remove(&model_id);
            }
            return Err("Download cancelled".to_string());
        }

        let file_path = model_dir.join(&file.filename);

        // Make request
        let response = client
            .get(&file.url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let _ = std::fs::remove_dir_all(&model_dir);
            {
                let mut downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
                downloads.remove(&model_id);
            }
            {
                let mut progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;
                progress.remove(&model_id);
            }
            return Err(format!("HTTP error: {}", response.status()));
        }

        // Create file
        let mut out_file = tokio::fs::File::create(&file_path)
            .await
            .map_err(|e| e.to_string())?;

        // Stream download
        let mut stream = response.bytes_stream();
        let mut file_downloaded: u64 = 0;

        while let Some(chunk_result) = stream.next().await {
            if cancel_token.load(Ordering::SeqCst) {
                drop(out_file);
                let _ = std::fs::remove_dir_all(&model_dir);
                {
                    let mut downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
                    downloads.remove(&model_id);
                }
                {
                    let mut progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;
                    progress.remove(&model_id);
                }
                return Err("Download cancelled".to_string());
            }

            let chunk = chunk_result.map_err(|e| e.to_string())?;
            out_file.write_all(&chunk).await.map_err(|e| e.to_string())?;

            file_downloaded += chunk.len() as u64;
            total_downloaded += chunk.len() as u64;

            let percent = (total_downloaded as f32 / total_bytes as f32) * 100.0;
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed_bps = if elapsed > 0.0 {
                (total_downloaded as f64 / elapsed) as u64
            } else {
                0
            };

            // Update progress
            {
                let mut progress_map = state.downloading_progress.lock().map_err(|e| e.to_string())?;
                progress_map.insert(model_id.clone(), percent);
            }

            // Emit progress event (throttled to every 100ms worth of data)
            if file_downloaded % (1024 * 100) < chunk.len() as u64 {
                let _ = app.emit(
                    "download:progress",
                    DownloadProgressPayload {
                        model_id: model_id.clone(),
                        downloaded_bytes: total_downloaded,
                        total_bytes,
                        percent,
                        speed_bps,
                    },
                );
            }
        }

        out_file.flush().await.map_err(|e| e.to_string())?;
    }

    // Cleanup tracking
    {
        let mut downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
        downloads.remove(&model_id);
    }
    {
        let mut progress = state.downloading_progress.lock().map_err(|e| e.to_string())?;
        progress.remove(&model_id);
    }

    // Emit completion
    let _ = app.emit("download:complete", model_id.clone());
    println!("[download] Completed: {}", model_id);

    Ok(())
}

#[tauri::command]
pub fn cancel_download(
    args: CancelDownloadArgs,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let model_id = args.model_id;

    let downloads = state.active_downloads.lock().map_err(|e| e.to_string())?;
    if let Some(cancel_token) = downloads.get(&model_id) {
        cancel_token.store(true, Ordering::SeqCst);
        println!("[download] Cancelled: {}", model_id);
    }

    Ok(())
}

#[tauri::command]
pub fn delete_model(
    args: DeleteModelArgs,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let model_id = args.model_id;

    // Cannot delete current model
    {
        let current = state.current_model_id.lock().map_err(|e| e.to_string())?;
        if current.as_ref() == Some(&model_id) {
            return Err("Cannot delete the currently active model".to_string());
        }
    }

    // Cannot delete legacy model this way
    if model_id == "legacy" {
        return Err("Cannot delete legacy model through this interface".to_string());
    }

    // Delete model directory
    let model_dir = get_model_dir(&state.models_dir, &model_id);
    if model_dir.exists() {
        std::fs::remove_dir_all(&model_dir).map_err(|e| e.to_string())?;
        println!("[model] Deleted: {}", model_id);
    }

    Ok(())
}
