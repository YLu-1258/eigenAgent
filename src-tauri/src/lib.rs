// src-tauri/src/lib.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest_eventsource::{Event, EventSource};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;
use tokio::io::AsyncWriteExt;

const MAX_TOKENS: u32 = 8192;
const SERVER_PORT: u16 = 8080;

const SYSTEM_PROMPT: &str = r#"
You are Eigen, a helpful AI assistant.

Rules:
- Use Markdown for formatting.
- Use LaTeX ($...$ / $$...$$) for math.
- If you don't know, say "I don't know".
"#;

// ==================== Data Structures ====================

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Serialize)]
pub struct ChatListItem {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub preview: String,
}

#[derive(Serialize)]
pub struct ChatMessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub thinking: String,
    pub images: Vec<String>,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Deserialize)]
struct ChatStreamArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    chat_id: String,
    prompt: String,
    #[serde(default)]
    images: Vec<String>,
}

#[derive(Deserialize)]
struct RenameChatArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    chat_id: String,
    title: String,
}

#[derive(Deserialize)]
struct DeleteChatArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    chat_id: String,
}

#[derive(Clone, Serialize)]
pub struct ChatBeginPayload {
    pub chat_id: String,
}

#[derive(Clone, Serialize)]
pub struct ChatDeltaPayload {
    pub chat_id: String,
    pub delta: String,
    pub reasoning_delta: String,
}

#[derive(Clone, Serialize)]
pub struct ChatEndPayload {
    pub chat_id: String,
    pub duration_ms: i64,
}

// ==================== Model Catalog Structures ====================

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
struct SwitchModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    model_id: String,
}

#[derive(Deserialize)]
struct DownloadModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    model_id: String,
}

#[derive(Deserialize)]
struct CancelDownloadArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    model_id: String,
}

#[derive(Deserialize)]
struct DeleteModelArgs {
    #[serde(alias = "model_id", alias = "modelId")]
    model_id: String,
}

// ==================== Server Manager ====================

struct LlamaServerManager {
    process: Mutex<Option<CommandChild>>,
    server_url: String,
    is_ready: AtomicBool,
    is_cancelled: AtomicBool,
    db_path: PathBuf,
    models_dir: PathBuf,
    model_path: Mutex<PathBuf>,
    mmproj_path: Mutex<Option<PathBuf>>,
    current_model_id: Mutex<Option<String>>,
    active_downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
    downloading_progress: Mutex<HashMap<String, f32>>,
}

// ==================== OpenAI API Types ====================

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    max_tokens: u32,
}

#[derive(Serialize, Clone)]
struct OpenAIMessage {
    role: String,
    content: OpenAIContent,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
enum OpenAIContent {
    Text(String),
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
enum OpenAIContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlData },
}

#[derive(Serialize, Clone)]
struct ImageUrlData {
    url: String,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamResponse {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamChoice {
    delta: OpenAIDelta,
}

#[derive(Deserialize, Debug)]
struct OpenAIDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

// ==================== Model File Discovery ====================

/// Scans a directory for .gguf model files.
/// Returns (main_model, optional_mmproj) if found.
fn scan_models_dir(models_dir: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
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

fn find_model_files(app: &AppHandle) -> Result<(PathBuf, Option<PathBuf>), String> {
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

// ==================== Model Catalog Functions ====================

fn get_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn get_catalog_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(get_models_dir(app)?.join("model-catalog.json"))
}

fn load_or_create_catalog(app: &AppHandle) -> Result<ModelCatalog, String> {
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

fn get_model_dir(models_dir: &Path, model_id: &str) -> PathBuf {
    models_dir.join(model_id)
}

fn is_model_downloaded(models_dir: &Path, entry: &ModelCatalogEntry) -> bool {
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

fn get_model_paths(models_dir: &Path, entry: &ModelCatalogEntry) -> Option<(PathBuf, Option<PathBuf>)> {
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

/// Detect legacy models in flat structure (for migration)
fn detect_legacy_model(models_dir: &Path) -> Option<String> {
    if let Some((model_path, _)) = scan_models_dir(models_dir) {
        // Check if this is a flat structure (not in a subdirectory)
        if model_path.parent() == Some(models_dir) {
            return Some("legacy".to_string());
        }
    }
    None
}

// ==================== Database Functions ====================

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as i64
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_millis(2000))
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn init_db(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS conversations (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            summary     TEXT NOT NULL DEFAULT '',
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            thinking        TEXT NOT NULL DEFAULT '',
            images          TEXT NOT NULL DEFAULT '[]',
            created_at      INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv_created
            ON messages(conversation_id, created_at);
        "#,
    )
    .map_err(|e| e.to_string())?;

    // Migration: add thinking column if missing
    let has_thinking: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'thinking'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_thinking {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN thinking TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // Migration: add images column if missing
    let has_images: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'images'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_images {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN images TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // Migration: add duration_ms column if missing
    let has_duration: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'duration_ms'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_duration {
        conn.execute("ALTER TABLE messages ADD COLUMN duration_ms INTEGER", [])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn insert_message(
    conn: &Connection,
    chat_id: &str,
    role: &str,
    content: &str,
    thinking: &str,
    images: &[String],
    duration_ms: Option<i64>,
) -> Result<(), String> {
    let now = unix_ms();
    let msg_id = uuid::Uuid::new_v4().to_string();
    let images_json = serde_json::to_string(images).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, thinking, images, created_at, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            msg_id,
            chat_id,
            role,
            content,
            thinking,
            images_json,
            now,
            duration_ms,
        ],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn resolve_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("eigenAgent.sqlite3"))
}

// ==================== Server Lifecycle ====================

async fn wait_for_server_ready(url: &str, timeout_secs: u64) -> Result<(), String> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/health", url);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err("Server startup timeout".to_string());
        }

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

// ==================== Tauri Commands ====================

#[tauri::command]
fn model_status(state: State<'_, LlamaServerManager>) -> Result<bool, String> {
    Ok(state.is_ready.load(Ordering::SeqCst))
}

#[tauri::command]
fn new_chat(app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<String, String> {
    let chat_id = uuid::Uuid::new_v4().to_string();
    let now = unix_ms();

    let conn = open_db(&state.db_path)?;
    conn.execute(
        "INSERT INTO conversations (id, title, summary, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat_id, "New chat", "", now, now],
    )
    .map_err(|e| e.to_string())?;

    let _ = app.emit("chats:changed", ());
    Ok(chat_id)
}

#[tauri::command]
fn list_chats(state: State<'_, LlamaServerManager>) -> Result<Vec<ChatListItem>, String> {
    let conn = open_db(&state.db_path)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                c.id,
                c.title,
                c.updated_at,
                COALESCE(
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.conversation_id = c.id
                     ORDER BY m.created_at DESC
                     LIMIT 1),
                    ''
                ) AS preview
            FROM conversations c
            ORDER BY c.updated_at DESC
            LIMIT 100
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ChatListItem {
                id: row.get(0)?,
                title: row.get(1)?,
                updated_at: row.get(2)?,
                preview: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
fn get_chat_messages(
    chat_id: String,
    state: State<'_, LlamaServerManager>,
) -> Result<Vec<ChatMessageRow>, String> {
    let conn = open_db(&state.db_path)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, role, content, thinking, images, created_at, duration_ms
            FROM messages
            WHERE conversation_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([chat_id], |row| {
            let images_json: String = row.get(4)?;
            let images: Vec<String> =
                serde_json::from_str(&images_json).unwrap_or_else(|_| Vec::new());

            Ok(ChatMessageRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                thinking: row.get(3)?,
                images,
                created_at: row.get(5)?,
                duration_ms: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
fn rename_chat(args: RenameChatArgs, state: State<'_, LlamaServerManager>) -> Result<(), String> {
    let conn = open_db(&state.db_path)?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![args.title, unix_ms(), args.chat_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_chat(args: DeleteChatArgs, app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<(), String> {
    let conn = open_db(&state.db_path)?;
    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        params![args.chat_id.clone()],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        params![args.chat_id],
    )
    .map_err(|e| e.to_string())?;

    let _ = app.emit("chats:changed", ());
    Ok(())
}

#[tauri::command]
fn cancel_generation(state: State<'_, LlamaServerManager>) -> Result<(), String> {
    state.is_cancelled.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn chat_stream(
    args: ChatStreamArgs,
    app: AppHandle,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let chat_id = args.chat_id;
    let prompt = args.prompt;
    let images = args.images;

    let start_time = Instant::now();

    // Reset cancellation flag
    state.is_cancelled.store(false, Ordering::SeqCst);

    // Save user message immediately
    {
        let conn = open_db(&state.db_path)?;
        insert_message(&conn, &chat_id, "user", &prompt, "", &images, None)?;
    }

    // Load conversation history
    let history_msgs = {
        let conn = open_db(&state.db_path)?;

        let mut stmt = conn
            .prepare(
                r#"
                SELECT role, content, images
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![chat_id.clone()], |row| {
                let images_json: String = row.get(2)?;
                let images: Vec<String> =
                    serde_json::from_str(&images_json).unwrap_or_else(|_| Vec::new());

                Ok(ChatMsg {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    images,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut msgs = Vec::new();
        for r in rows {
            msgs.push(r.map_err(|e| e.to_string())?);
        }
        msgs
    };

    // Build OpenAI-format messages
    let mut openai_messages: Vec<OpenAIMessage> = vec![OpenAIMessage {
        role: "system".to_string(),
        content: OpenAIContent::Text(SYSTEM_PROMPT.to_string()),
    }];

    // Add recent history (last 20 turns)
    let recent = if history_msgs.len() > 20 {
        &history_msgs[history_msgs.len() - 20..]
    } else {
        &history_msgs[..]
    };

    for msg in recent {
        let content = if msg.images.is_empty() {
            OpenAIContent::Text(msg.content.clone())
        } else {
            let mut parts: Vec<OpenAIContentPart> = vec![OpenAIContentPart::Text {
                text: msg.content.clone(),
            }];

            for img_base64 in &msg.images {
                parts.push(OpenAIContentPart::ImageUrl {
                    image_url: ImageUrlData {
                        url: format!("data:image/jpeg;base64,{}", img_base64),
                    },
                });
            }

            OpenAIContent::Parts(parts)
        };

        openai_messages.push(OpenAIMessage {
            role: msg.role.clone(),
            content,
        });
    }

    // Emit stream begin
    app.emit(
        "chat:begin",
        ChatBeginPayload {
            chat_id: chat_id.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    // Make streaming request to llama-server
    let client = reqwest::Client::new();
    let request_body = OpenAIRequest {
        model: "qwen3-vl".to_string(),
        messages: openai_messages,
        stream: true,
        max_tokens: MAX_TOKENS,
    };

    let request_builder = client
        .post(format!("{}/v1/chat/completions", state.server_url))
        .header("Content-Type", "application/json")
        .json(&request_body);

    let mut es = EventSource::new(request_builder).map_err(|e| e.to_string())?;
    let mut full_response_content = String::new();
    let mut full_response_thinking = String::new();

    while let Some(event) = es.next().await {
        if state.is_cancelled.load(Ordering::SeqCst) {
            es.close();
            break;
        }

        match event {
            Ok(Event::Open) => {}
            Ok(Event::Message(msg)) => {
                if msg.data == "[DONE]" {
                    break;
                }
                
                if let Ok(parsed) = serde_json::from_str::<OpenAIStreamResponse>(&msg.data) {
                    print!("{0}", msg.data);
                    if let Some(choice) = parsed.choices.first() {
                        let content_delta = choice.delta.content.clone().unwrap_or_default();
                        let reasoning_delta = choice.delta.reasoning_content.clone().unwrap_or_default();

                        if !content_delta.is_empty() {
                            full_response_content.push_str(&content_delta);
                        }
                        if !reasoning_delta.is_empty() {
                            full_response_thinking.push_str(&reasoning_delta);
                        }
                        
                        app.emit(
                            "chat:delta",
                            ChatDeltaPayload {
                                chat_id: chat_id.clone(),
                                delta: content_delta,
                                reasoning_delta,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
            Err(e) => {
                eprintln!("[SSE Error] {:?}", e);
                break;
            }
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as i64;

    // Save assistant response
    {
        let conn = open_db(&state.db_path)?;
        insert_message(
            &conn,
            &chat_id,
            "assistant",
            &full_response_content,
            &full_response_thinking,
            &[],
            Some(duration_ms),
        )?;
    }

    // Emit stream end
    app.emit(
        "chat:end",
        ChatEndPayload {
            chat_id: chat_id.clone(),
            duration_ms,
        },
    )
    .map_err(|e| e.to_string())?;

    let _ = app.emit("chats:changed", ());

    Ok(())
}

// ==================== Model Catalog Commands ====================

#[tauri::command]
fn list_models(app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<Vec<ModelInfo>, String> {
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
fn get_current_model(state: State<'_, LlamaServerManager>) -> Result<Option<String>, String> {
    let current = state.current_model_id.lock().map_err(|e| e.to_string())?;
    Ok(current.clone())
}

#[tauri::command]
async fn switch_model(
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

    cmd = cmd
        .args(["-m", model_path.to_str().unwrap()])
        .args(["--host", "127.0.0.1"])
        .args(["--port", &SERVER_PORT.to_string()])
        .args(["--ctx-size", "8192"])
        .args(["--n-predict", &MAX_TOKENS.to_string()]);

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
async fn download_model(
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
fn cancel_download(
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
fn delete_model(
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

            // Load or create model catalog
            let catalog = load_or_create_catalog(&app_handle)?;
            println!("[catalog] loaded {} models", catalog.models.len());

            // Find model files - try catalog first, then legacy
            let found_model: Option<(PathBuf, Option<PathBuf>, String)> = {
                // First try to find a downloaded model from catalog
                let mut found: Option<(PathBuf, Option<PathBuf>, String)> = None;

                for entry in &catalog.models {
                    if let Some((mp, mmpp)) = get_model_paths(&models_dir, entry) {
                        found = Some((mp, mmpp, entry.id.clone()));
                        break;
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

                    cmd = cmd
                        .args(["-m", model_path_clone.to_str().unwrap()])
                        .args(["--host", "127.0.0.1"])
                        .args(["--port", &SERVER_PORT.to_string()])
                        .args(["--ctx-size", "8192"])
                        .args(["--n-predict", &MAX_TOKENS.to_string()]);

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
            delete_chat,
            cancel_generation,
            chat_stream,
            list_models,
            get_current_model,
            switch_model,
            download_model,
            cancel_download,
            delete_model
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}