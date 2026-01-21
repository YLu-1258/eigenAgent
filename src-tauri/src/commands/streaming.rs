// src-tauri/src/commands/streaming.rs

use std::sync::atomic::Ordering;
use std::time::Instant;

use futures::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use rusqlite::params;
use tauri::{AppHandle, Emitter, State};

use crate::db::{insert_message, open_db};
use crate::state::{LlamaServerManager, MAX_TOKENS};
use crate::types::{
    ChatBeginPayload, ChatDeltaPayload, ChatEndPayload, ChatMsg, ChatStreamArgs,
    ImageUrlData, OpenAIContent, OpenAIContentPart, OpenAIMessage, OpenAIRequest,
    OpenAIStreamResponse,
};

#[tauri::command]
pub async fn chat_stream(
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

    // Get system prompt from settings
    let system_prompt = {
        let settings = state.app_settings.lock().map_err(|e| e.to_string())?;
        settings.defaults.system_prompt.clone()
    };

    // Build OpenAI-format messages
    let mut openai_messages: Vec<OpenAIMessage> = vec![OpenAIMessage {
        role: "system".to_string(),
        content: OpenAIContent::Text(system_prompt),
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
                    if let Some(choice) = parsed.choices.first() {
                        let content_delta = choice.delta.content.clone().unwrap_or_default();
                        let reasoning_delta = choice.delta.reasoning_content.clone().unwrap_or_default();

                        if !content_delta.is_empty() {
                            full_response_content.push_str(&content_delta);
                            print!("{}", content_delta);
                        }
                        if !reasoning_delta.is_empty() {
                            full_response_thinking.push_str(&reasoning_delta);
                            print!("{}", reasoning_delta)
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
