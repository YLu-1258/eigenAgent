// src-tauri/src/lib.rs

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use tauri::{AppHandle, Emitter, Manager, State};
use serde::Deserialize;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;

const MODEL_PATH: &str = "/Users/alexa/Projects/eigen/eigenAgent/models/Qwen3VL-4B-Thinking-Q4_K_M.gguf";
const MAX_TOKENS: u32 = 4096;

const SYSTEM_PROMPT: &str = r#"
You are Eigen, a helpful AI assistant.

Rules:
- Be concise. Prefer short answers by default.
- Do NOT include long internal reasoning.
- If you use <think>...</think>, keep it to 1–3 short sentences max.
- Use Markdown for formatting.
- Use LaTeX ($...$ / $$...$$) for math.
- If you don't know, say "I don't know".
"#;


#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMsg {
    pub role: String,   // "user" | "assistant"
    pub content: String // assistant final answer only
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    pub id: String,
    pub messages: Vec<ChatMsg>,
    pub summary: String,
}

#[derive(serde::Serialize)]
pub struct ChatListItem {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub preview: String,
}

#[derive(serde::Serialize)]
pub struct ChatMessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Deserialize)]
struct ChatStreamArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    chat_id: String,
    prompt: String,
}

#[derive(Deserialize)]
struct ChatIdArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    chat_id: String,
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

#[derive(Clone, serde::Serialize)]
pub struct ChatBeginPayload {
    pub chat_id: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ChatDeltaPayload {
    pub chat_id: String,
    pub delta: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ChatEndPayload {
    pub chat_id: String,
}

struct EigenBrain {
    model: Mutex<Option<LlamaModel>>,
    backend: LlamaBackend,
    is_loaded: AtomicBool,
    db_path: PathBuf,
}

fn unix_ms() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    now.as_millis() as i64
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;

    // Make SQLite friendlier for app usage
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| e.to_string())?;
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
            created_at      INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv_created
            ON messages(conversation_id, created_at);
        "#,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn utf8_tail(s: &str, max_bytes: usize) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut start = s.len().saturating_sub(max_bytes);
    // Walk forward/backward to a valid char boundary
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    s[start..].to_string()
}

fn build_prompt(system: &str, summary: &str, msgs: &[ChatMsg], max_turns: usize) -> String {
    let recent = if msgs.len() > max_turns {
        &msgs[msgs.len() - max_turns..]
    } else {
        msgs
    };

    let mut out = String::new();

    out.push_str(&format!("<|im_start|>system\n{}<|im_end|>\n", system));

    if !summary.trim().is_empty() {
        out.push_str(&format!(
            "<|im_start|>system\nConversation summary:\n{}<|im_end|>\n",
            summary
        ));
    }

    for m in recent {
        if m.role == "user" {
            out.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", m.content));
        } else {
            out.push_str(&format!("<|im_start|>assistant\n{}<|im_end|>\n", m.content));
        }
    }

    out.push_str("<|im_start|>assistant\n");
    out
}

fn insert_message(conn: &Connection, chat_id: &str, role: &str, content: &str) -> Result<(), String> {
    let now = unix_ms();
    let msg_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![msg_id, chat_id, role, content, now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn model_status(state: State<'_, EigenBrain>) -> Result<bool, String> {
    Ok(state.is_loaded.load(Ordering::SeqCst))
}

#[tauri::command]
fn new_chat(app: AppHandle, state: State<'_, EigenBrain>) -> Result<String, String> {
    let chat_id = uuid::Uuid::new_v4().to_string();
    let now = unix_ms();

    let conn = open_db(&state.db_path)?;
    conn.execute(
        "INSERT INTO conversations (id, title, summary, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat_id, "New chat", "", now, now],
    )
    .map_err(|e| e.to_string())?;

    // Optional: you can emit an event to refresh sidebar, but frontend can just call list_chats.
    let _ = app.emit("chats:changed", ());

    Ok(chat_id)
}

#[tauri::command]
fn list_chats(state: State<'_, EigenBrain>) -> Result<Vec<ChatListItem>, String> {
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
fn get_chat_messages(args: ChatIdArgs, state: State<'_, EigenBrain>) -> Result<Vec<ChatMessageRow>, String> {
    let conn = open_db(&state.db_path)?;
    let chat_id = args.chat_id;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, role, content, created_at
            FROM messages
            WHERE conversation_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([chat_id], |row| {
            Ok(ChatMessageRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
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
fn rename_chat(args: RenameChatArgs, state: State<'_, EigenBrain>) -> Result<(), String> {
    let conn = open_db(&state.db_path)?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![args.title, unix_ms(), args.chat_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}


#[tauri::command]
fn delete_chat(args: DeleteChatArgs, state: State<'_, EigenBrain>) -> Result<(), String> {
    let conn = open_db(&state.db_path)?;
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1", params![args.chat_id.clone()])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![args.chat_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}


#[tauri::command]
async fn chat_stream(
    args: ChatStreamArgs,
    app: AppHandle,
    state: State<'_, EigenBrain>,
) -> Result<(), String> {
    let chat_id = args.chat_id;
    let prompt = args.prompt;

    // 0) Ensure model exists
    let model_guard = state.model.lock().map_err(|_| "Failed to lock model".to_string())?;
    let model = model_guard.as_ref().ok_or_else(|| "No model loaded".to_string())?;

    // 1) Save user message immediately
    {
        let conn = open_db(&state.db_path)?;
        insert_message(&conn, &chat_id, "user", &prompt)?;
    }

    // 2) Load summary + history for prompt building
    let (summary, history_msgs) = {
        let conn = open_db(&state.db_path)?;

        let summary: String = conn
            .query_row(
                "SELECT summary FROM conversations WHERE id = ?1",
                params![chat_id.clone()],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                r#"
                SELECT role, content
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![chat_id.clone()], |row| {
                Ok(ChatMsg {
                    role: row.get(0)?,
                    content: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut msgs = Vec::new();
        for r in rows {
            msgs.push(r.map_err(|e| e.to_string())?);
        }

        (summary, msgs)
    };

    // 3) Build prompt from DB history
    let formatted_prompt = build_prompt(SYSTEM_PROMPT, &summary, &history_msgs, 20);

    // 4) Tokenize
    let mut tokens = model
        .str_to_token(&formatted_prompt, AddBos::Always)
        .map_err(|e| e.to_string())?;

    // 5) Context
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(NonZeroU32::new(MAX_TOKENS).unwrap()));
    let mut ctx = model
        .new_context(&state.backend, ctx_params)
        .map_err(|e| e.to_string())?;

    // 6) Prefill (logits only on last prompt token)
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    for (i, &tok) in tokens.iter().enumerate() {
        let want_logits = i + 1 == tokens.len();
        batch.add(tok, i as i32, &[0], want_logits)
            .map_err(|e| e.to_string())?;
    }
    ctx.decode(&mut batch).map_err(|e| e.to_string())?;

    let mut sampler = LlamaSampler::greedy();
    let mut logits_i = (tokens.len() - 1) as i32;

    // 7) Stream begin (include chat_id so UI can route it)
    app.emit(
        "chat:begin",
        ChatBeginPayload {
            chat_id: chat_id.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    // Collect final answer (excluding <think> blocks)
    let mut final_answer = String::new();
    let mut utf8_buf: Vec<u8> = Vec::new();

    let mut in_think = false;
    let mut carry = String::new(); // handles tags split across chunks

    for _ in 0..MAX_TOKENS {
        let next = sampler.sample(&ctx, logits_i);
        if next == model.token_eos() {
            break;
        }

        let bytes = model
            .token_to_bytes(next, Special::Plaintext)
            .map_err(|e| e.to_string())?;
        utf8_buf.extend_from_slice(&bytes);

        // Only emit when we have valid UTF-8
        match std::str::from_utf8(&utf8_buf) {
            Ok(valid_str) => {
                // Send to UI
                app.emit(
                    "chat:delta",
                    ChatDeltaPayload {
                        chat_id: chat_id.clone(),
                        delta: valid_str.to_string(),
                    },
                )
                .map_err(|e| e.to_string())?;

                // Parse into final_answer without <think>...</think>
                let mut chunk = String::new();
                chunk.push_str(&carry);
                chunk.push_str(valid_str);
                carry.clear();

                let mut i = 0;
                while i < chunk.len() {
                    if !in_think {
                        if let Some(start) = chunk[i..].find("<think>") {
                            final_answer.push_str(&chunk[i..i + start]);
                            in_think = true;
                            i = i + start + "<think>".len();
                        } else {
                            final_answer.push_str(&chunk[i..]);
                            break;
                        }
                    } else {
                        if let Some(end) = chunk[i..].find("</think>") {
                            in_think = false;
                            i = i + end + "</think>".len();
                        } else {
                            // drop remainder while in think
                            break;
                        }
                    }
                }

                carry = utf8_tail(&chunk, 16);
                utf8_buf.clear();
            }
            Err(e) => {
                // incomplete sequence -> keep buffering
                if e.error_len().is_none() {
                    // do nothing
                } else {
                    // invalid bytes -> emit lossy and reset to recover
                    let lossy = String::from_utf8_lossy(&utf8_buf).to_string();
                    app.emit(
                        "chat:delta",
                        ChatDeltaPayload {
                            chat_id: chat_id.clone(),
                            delta: lossy,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    utf8_buf.clear();
                }
            }
        }

        // Decode step token (1-token batch => logits index 0)
        tokens.push(next);
        let pos = (tokens.len() - 1) as i32;

        let mut step_batch = LlamaBatch::new(1, 1);
        step_batch.add(next, pos, &[0], true).map_err(|e| e.to_string())?;
        ctx.decode(&mut step_batch).map_err(|e| e.to_string())?;

        logits_i = 0;
    }

    // 8) Save assistant final answer
    {
        let conn = open_db(&state.db_path)?;
        insert_message(&conn, &chat_id, "assistant", &final_answer)?;
    }

    // 9) Stream end
    app.emit(
        "chat:end",
        ChatEndPayload {
            chat_id: chat_id.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    // Optional: notify sidebar refresh
    let _ = app.emit("chats:changed", ());

    Ok(())
}

fn resolve_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    // Tauri v2 style path resolver
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("eigenAgent.sqlite3"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let backend = LlamaBackend::init().expect("Failed to init llama backend");

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Resolve DB path + init schema
            let db_path = resolve_db_path(&app_handle)?;
            {
                println!("[db] path = {}", db_path.display());

                let conn = open_db(&db_path)?;
                init_db(&conn)?;
            }

            // Store state
            app.manage(EigenBrain {
                model: Mutex::new(None),
                backend,
                is_loaded: AtomicBool::new(false),
                db_path,
            });

            // Emit model loading
            let _ = app_handle.emit("model:loading", ());

            // Load model in background
            tauri::async_runtime::spawn_blocking(move || {
                let state = app_handle.state::<EigenBrain>();

                let result: Result<(), String> = (|| {
                    let model = LlamaModel::load_from_file(
                        &state.backend,
                        MODEL_PATH,
                        &LlamaModelParams::default(),
                    )
                    .map_err(|e| e.to_string())?;

                    *state
                        .model
                        .lock()
                        .map_err(|_| "lock failed".to_string())? = Some(model);

                    state.is_loaded.store(true, Ordering::SeqCst);
                    Ok(())
                })();

                match result {
                    Ok(()) => {
                        let _ = app_handle.emit("model:ready", ());
                    }
                    Err(err) => {
                        let _ = app_handle.emit("model:error", err);
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
            chat_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
