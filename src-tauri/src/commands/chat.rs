// src-tauri/src/commands/chat.rs

use std::sync::atomic::Ordering;

use rusqlite::params;
use tauri::{AppHandle, Emitter, State};

use crate::db::{open_db, unix_ms};
use crate::state::LlamaServerManager;
use crate::types::{
    ChatListItem, ChatMessageRow, DeleteChatArgs, GenerateTitleArgs, RenameChatArgs,
    OpenAIContent, OpenAIMessage, OpenAINonStreamResponse, OpenAIRequest,
};

#[tauri::command]
pub fn model_status(state: State<'_, LlamaServerManager>) -> Result<bool, String> {
    Ok(state.is_ready.load(Ordering::SeqCst))
}

#[tauri::command]
pub fn new_chat(app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<String, String> {
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
pub fn list_chats(state: State<'_, LlamaServerManager>) -> Result<Vec<ChatListItem>, String> {
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
pub fn get_chat_messages(
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
pub fn rename_chat(args: RenameChatArgs, state: State<'_, LlamaServerManager>) -> Result<(), String> {
    let conn = open_db(&state.db_path)?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![args.title, unix_ms(), args.chat_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn generate_chat_title(
    args: GenerateTitleArgs,
    app: AppHandle,
    state: State<'_, LlamaServerManager>,
) -> Result<(), String> {
    let chat_id = args.chat_id;

    // Check if server is ready
    if !state.is_ready.load(Ordering::SeqCst) {
        eprintln!("[generate_chat_title] Server not ready, skipping");
        return Ok(());
    }

    // Get the first user message from this chat
    let first_message = {
        let conn = open_db(&state.db_path)?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT content FROM messages
                WHERE conversation_id = ?1 AND role = 'user'
                ORDER BY created_at ASC
                LIMIT 1
                "#,
            )
            .map_err(|e| e.to_string())?;

        let content: Option<String> = stmt
            .query_row(params![chat_id.clone()], |row| row.get(0))
            .ok();

        content
    };

    let first_message = match first_message {
        Some(msg) => msg,
        None => return Ok(()), // No user message yet, nothing to do
    };

    // Truncate message if too long (for efficiency)
    let truncated_msg = if first_message.len() > 300 {
        format!("{}...", &first_message[..300])
    } else {
        first_message
    };

    // Use LLM to generate a concise title
    let client = reqwest::Client::new();

    let request_body = OpenAIRequest {
        model: "default".to_string(),
        messages: vec![
            OpenAIMessage {
                role: "system".to_string(),
                content: OpenAIContent::Text(
                    "Generate a short chat title (3-6 words max). Return ONLY the title, no quotes, no explanation.".to_string()
                ),
            },
            OpenAIMessage {
                role: "user".to_string(),
                content: OpenAIContent::Text(truncated_msg),
            },
        ],
        stream: false,
        max_tokens: 30,
    };

    let response = match client
        .post(format!("{}/v1/chat/completions", state.server_url))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("[generate_chat_title] Request failed: {}", e);
            return Ok(());
        }
    };

    if !response.status().is_success() {
        eprintln!("[generate_chat_title] HTTP error: {}", response.status());
        return Ok(());
    }

    let response_body: OpenAINonStreamResponse = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            eprintln!("[generate_chat_title] Failed to parse response: {}", e);
            return Ok(());
        }
    };

    let generated_title = response_body
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "New chat".to_string());

    // Clean up the title: remove quotes, trim, limit length
    let final_title = generated_title
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .lines()
        .next()
        .unwrap_or("New chat")
        .chars()
        .take(80)
        .collect::<String>();

    let final_title = if final_title.is_empty() {
        "New chat".to_string()
    } else {
        final_title
    };

    eprintln!("[generate_chat_title] Generated title: {:?}", final_title);

    // Update the chat title in the database
    {
        let conn = open_db(&state.db_path)?;
        conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![final_title, unix_ms(), chat_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Notify frontend that chats have changed
    let _ = app.emit("chats:changed", ());

    Ok(())
}

#[tauri::command]
pub fn delete_chat(args: DeleteChatArgs, app: AppHandle, state: State<'_, LlamaServerManager>) -> Result<(), String> {
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
pub fn cancel_generation(state: State<'_, LlamaServerManager>) -> Result<(), String> {
    state.is_cancelled.store(true, Ordering::SeqCst);
    Ok(())
}
