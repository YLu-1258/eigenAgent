// src-tauri/src/lib.rs

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

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

// ==================== Server Manager ====================

struct LlamaServerManager {
    process: Mutex<Option<CommandChild>>,
    server_url: String,
    is_ready: AtomicBool,
    is_cancelled: AtomicBool,
    db_path: PathBuf,
    model_path: PathBuf,
    mmproj_path: Option<PathBuf>,
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

fn find_model_files(app: &AppHandle) -> Result<(PathBuf, Option<PathBuf>), String> {
    let models_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|p| p.join("../../../models"))
        .unwrap();

    if !models_dir.exists() {
        return Err(format!(
            "Models directory not found: {}",
            models_dir.display()
        ));
    }

    let entries = std::fs::read_dir(&models_dir)
        .map_err(|e| format!("Failed to read models directory: {}", e))?;

    let mut main_model: Option<PathBuf> = None;
    let mut mmproj: Option<PathBuf> = None;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
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

    let model = main_model.ok_or_else(|| "No main .gguf model file found".to_string())?;
    Ok((model, mmproj))
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

            // Find model files
            let (model_path, mmproj_path) = find_model_files(&app_handle)?;
            println!("[model] Main model: {}", model_path.display());
            if let Some(ref mmproj) = mmproj_path {
                println!("[model] Vision projector: {}", mmproj.display());
            }

            let server_url = format!("http://127.0.0.1:{}", SERVER_PORT);

            // Store state
            app.manage(LlamaServerManager {
                process: Mutex::new(None),
                server_url: server_url.clone(),
                is_ready: AtomicBool::new(false),
                is_cancelled: AtomicBool::new(false),
                db_path,
                model_path: model_path.clone(),
                mmproj_path: mmproj_path.clone(),
            });

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
            chat_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}