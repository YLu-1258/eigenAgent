// src-tauri/src/lib.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};
use tauri_plugin_shell::ShellExt;

mod commands;
mod db;
mod models;
mod server;
mod settings;
mod state;
mod types;

use commands::{
    cancel_download, cancel_generation, chat_stream, delete_chat, delete_model,
    download_model, generate_chat_title, get_chat_messages, get_current_model, list_chats,
    list_models, model_status, new_chat, rename_chat, switch_model,
};
use db::{init_db, open_db, resolve_db_path};
use models::{find_model_files, get_model_paths, get_models_dir, load_or_create_catalog, scan_models_dir};
use server::wait_for_server_ready;
use settings::{get_default_settings, load_settings, save_settings, AppSettings};
use state::{LlamaServerManager, SERVER_PORT};

// ==================== Settings Commands ====================

#[tauri::command]
fn cmd_load_settings(state: tauri::State<'_, LlamaServerManager>) -> Result<AppSettings, String> {
    let settings = state.app_settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
fn cmd_save_settings(
    new_settings: AppSettings,
    state: tauri::State<'_, LlamaServerManager>,
) -> Result<(), String> {
    // Save to disk
    save_settings(&new_settings)?;

    // Update in-memory state
    let mut settings = state.app_settings.lock().map_err(|e| e.to_string())?;
    *settings = new_settings;

    println!("[settings] Settings updated");
    Ok(())
}

#[tauri::command]
fn cmd_reset_settings(state: tauri::State<'_, LlamaServerManager>) -> Result<AppSettings, String> {
    let default_settings = get_default_settings();

    // Save defaults to disk
    save_settings(&default_settings)?;

    // Update in-memory state
    let mut settings = state.app_settings.lock().map_err(|e| e.to_string())?;
    *settings = default_settings.clone();

    println!("[settings] Settings reset to defaults");
    Ok(default_settings)
}

// ==================== App Entry Point ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Resolve DB path + init schema
            let db_path = resolve_db_path(&app_handle)?;
            {
                println!("[db] path = {}", db_path.display());
                let conn = open_db(&db_path)?;
                init_db(&conn)?;
            }

            // Get models directory
            let models_dir = get_models_dir(&app_handle)?;
            println!("[models] dir = {}", models_dir.display());

            // Load settings first (needed for default model selection)
            let app_settings = load_settings().unwrap_or_else(|e| {
                eprintln!("[settings] Failed to load settings, using defaults: {}", e);
                get_default_settings()
            });
            println!("[settings] Loaded settings (theme: {})", app_settings.appearance.theme);

            // Load or create model catalog
            let catalog = load_or_create_catalog(&app_handle)?;
            println!("[catalog] loaded {} models", catalog.models.len());

            // Find model files - prefer default model from settings, then first available
            let found_model: Option<(PathBuf, Option<PathBuf>, String)> = {
                let mut found: Option<(PathBuf, Option<PathBuf>, String)> = None;

                // First, try to use the default model from settings if set
                if let Some(ref preferred_id) = app_settings.defaults.model_id {
                    println!("[model] Preferred model from settings: {}", preferred_id);
                    for entry in &catalog.models {
                        if entry.id == *preferred_id {
                            if let Some((mp, mmpp)) = get_model_paths(&models_dir, entry) {
                                println!("[model] Found preferred model: {}", entry.id);
                                found = Some((mp, mmpp, entry.id.clone()));
                                break;
                            }
                        }
                    }
                }

                // If preferred model not found, try first available from catalog
                if found.is_none() {
                    for entry in &catalog.models {
                        if let Some((mp, mmpp)) = get_model_paths(&models_dir, entry) {
                            found = Some((mp, mmpp, entry.id.clone()));
                            break;
                        }
                    }
                }

                // If no catalog model found, try legacy detection
                if found.is_none() {
                    if let Some((mp, mmpp)) = scan_models_dir(&models_dir) {
                        found = Some((mp, mmpp, "legacy".to_string()));
                    }
                }

                // If still not found, try development models folder
                if found.is_none() {
                    if let Ok((mp, mmpp)) = find_model_files(&app_handle) {
                        found = Some((mp, mmpp, "legacy".to_string()));
                    }
                }

                found
            };

            let server_url = format!("http://127.0.0.1:{}", SERVER_PORT);

            // Store state - use empty path if no model found
            let (model_path, mmproj_path, current_model_id) = match found_model {
                Some((mp, mmpp, id)) => {
                    println!("[model] Main model: {}", mp.display());
                    println!("[model] Current model ID: {}", id);
                    if let Some(ref mmproj) = mmpp {
                        println!("[model] Vision projector: {}", mmproj.display());
                    }
                    (mp, mmpp, Some(id))
                }
                None => {
                    println!("[model] No models found - app will start without a model");
                    (PathBuf::new(), None, None)
                }
            };

            let has_model = current_model_id.is_some();

            app.manage(LlamaServerManager {
                process: Mutex::new(None),
                server_url: server_url.clone(),
                is_ready: AtomicBool::new(false),
                is_cancelled: AtomicBool::new(false),
                db_path,
                models_dir,
                model_path: Mutex::new(model_path.clone()),
                mmproj_path: Mutex::new(mmproj_path.clone()),
                current_model_id: Mutex::new(current_model_id),
                active_downloads: Mutex::new(HashMap::new()),
                downloading_progress: Mutex::new(HashMap::new()),
                app_settings: Mutex::new(app_settings),
            });

            print!("[app] Do we have model: {}\n", has_model);

            // Only start the server if we have a model
            if has_model {
                // Emit model loading
                let _ = app_handle.emit("model:loading", ());

                // Spawn llama-server in background
                let model_path_clone = model_path.clone();
                let mmproj_path_clone = mmproj_path.clone();

                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<LlamaServerManager>();

                    // Build sidecar command
                    let shell = app_handle.shell();
                    let mut cmd = shell
                        .sidecar("llama-server")
                        .expect("Failed to create sidecar command");

                    // Get context length and max tokens from settings
                    let (ctx_size, max_tokens) = {
                        let settings = state.app_settings.lock().unwrap();
                        (
                            settings.behavior.context_length.to_string(),
                            settings.behavior.max_tokens.to_string(),
                        )
                    };

                    cmd = cmd
                        .args(["-m", model_path_clone.to_str().unwrap()])
                        .args(["--host", "127.0.0.1"])
                        .args(["--port", &SERVER_PORT.to_string()])
                        .args(["--ctx-size", &ctx_size])
                        .args(["--n-predict", &max_tokens]);

                    // Add vision projector if available
                    if let Some(ref mmproj) = mmproj_path_clone {
                        cmd = cmd.args(["--mmproj", mmproj.to_str().unwrap()]);
                    }

                    // Spawn the server
                    match cmd.spawn() {
                        Ok((mut rx, child)) => {
                            // Store the child process
                            if let Ok(mut guard) = state.process.lock() {
                                *guard = Some(child);
                            }

                            // Log server output in background
                            tauri::async_runtime::spawn(async move {
                                while let Some(event) = rx.recv().await {
                                    match event {
                                        tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                                            println!(
                                                "[llama-server] {}",
                                                String::from_utf8_lossy(&line)
                                            );
                                        }
                                        tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                                            eprintln!(
                                                "[llama-server] {}",
                                                String::from_utf8_lossy(&line)
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                            });

                            // Wait for server to be ready
                            match wait_for_server_ready(&state.server_url, 120).await {
                                Ok(()) => {
                                    state.is_ready.store(true, Ordering::SeqCst);
                                    let _ = app_handle.emit("model:ready", ());
                                    println!("[llama-server] Ready!");
                                }
                                Err(e) => {
                                    let _ = app_handle.emit("model:error", e);
                                }
                            }
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "model:error",
                                format!("Failed to spawn llama-server: {}", e),
                            );
                        }
                    }
                });
            } else {
                // Emit no_model event so frontend knows to show warning
                println!("[model] No model installed, emitting model:no_model event");
                let _ = app_handle.emit("model:no_model", ());
            }

            // Set up file watcher for models directory
            let models_dir_for_watcher = get_models_dir(&app.handle().clone())?;
            let app_handle_for_watcher = app.handle().clone();

            std::thread::spawn(move || {
                let (tx, rx) = std::sync::mpsc::channel();

                let mut watcher = match RecommendedWatcher::new(
                    move |res: Result<notify::Event, notify::Error>| {
                        if let Ok(event) = res {
                            // Only care about create/modify/remove events
                            match event.kind {
                                notify::EventKind::Create(_)
                                | notify::EventKind::Modify(_)
                                | notify::EventKind::Remove(_) => {
                                    let _ = tx.send(());
                                }
                                _ => {}
                            }
                        }
                    },
                    Config::default(),
                ) {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[watcher] Failed to create watcher: {}", e);
                        return;
                    }
                };

                if let Err(e) = watcher.watch(&models_dir_for_watcher, RecursiveMode::Recursive) {
                    eprintln!("[watcher] Failed to watch models dir: {}", e);
                    return;
                }

                println!("[watcher] Watching models directory: {}", models_dir_for_watcher.display());

                // Debounce: wait for events and batch them
                let mut last_emit = Instant::now();
                loop {
                    match rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(()) => {
                            // Debounce: only emit if at least 1 second since last emit
                            if last_emit.elapsed() > Duration::from_secs(1) {
                                println!("[watcher] Models directory changed, emitting event");
                                let _ = app_handle_for_watcher.emit("models:changed", ());
                                last_emit = Instant::now();
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            // No events, continue
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            println!("[watcher] Channel disconnected, stopping watcher");
                            break;
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            model_status,
            new_chat,
            list_chats,
            get_chat_messages,
            rename_chat,
            generate_chat_title,
            delete_chat,
            cancel_generation,
            chat_stream,
            list_models,
            get_current_model,
            switch_model,
            download_model,
            cancel_download,
            delete_model,
            cmd_load_settings,
            cmd_save_settings,
            cmd_reset_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
