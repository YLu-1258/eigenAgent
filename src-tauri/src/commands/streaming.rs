// src-tauri/src/commands/streaming.rs

use std::sync::atomic::Ordering;
use std::time::Instant;

use futures::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::db::{insert_message, open_db};
use crate::state::LlamaServerManager;
use crate::tools::{execute_tool, get_all_tools, tools_to_openai_format, ToolCallRequest};
use crate::types::{
    AssistantMessageWithToolCalls, ChatBeginPayload, ChatDeltaPayload, ChatEndPayload, ChatMsg,
    ChatStreamArgs, FunctionCall, ImageUrlData, OpenAIContent, OpenAIContentPart, OpenAIMessage,
    OpenAIRequest, OpenAIStreamResponse, ToolCall, ToolResultMessage,
};

// Event payloads for tool calling
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallingPayload {
    pub chat_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub call_id: String,
    pub arguments: Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub chat_id: String,
    pub call_id: String,
    pub tool_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

// Accumulated tool call during streaming
#[derive(Clone, Debug, Default)]
struct AccumulatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

// Generic message type for the conversation that can be serialized
#[derive(Serialize, Clone)]
#[serde(untagged)]
enum ConversationMessage {
    Standard(OpenAIMessage),
    AssistantWithTools(AssistantMessageWithToolCalls),
    ToolResult(ToolResultMessage),
}

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

    // Get settings
    let (system_prompt, max_tokens, enabled_tool_ids) = {
        let settings = state.app_settings.lock().map_err(|e| e.to_string())?;
        (
            settings.defaults.system_prompt.clone(),
            settings.behavior.max_tokens,
            settings.tools.enabled_tools.clone(),
        )
    };

    // Get enabled tools
    let enabled_tools: Vec<_> = get_all_tools()
        .into_iter()
        .filter(|t| enabled_tool_ids.contains(&t.id))
        .collect();

    let tools_json = if enabled_tools.is_empty() {
        None
    } else {
        Some(tools_to_openai_format(&enabled_tools))
    };

    // Build OpenAI-format messages
    let mut conversation_messages: Vec<ConversationMessage> =
        vec![ConversationMessage::Standard(OpenAIMessage {
            role: "system".to_string(),
            content: OpenAIContent::Text(system_prompt),
        })];

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

        conversation_messages.push(ConversationMessage::Standard(OpenAIMessage {
            role: msg.role.clone(),
            content,
        }));
    }

    // Emit stream begin
    app.emit(
        "chat:begin",
        ChatBeginPayload {
            chat_id: chat_id.clone(),
        },
    )
    .map_err(|e| e.to_string())?;

    let client = reqwest::Client::new();
    let mut full_response_content = String::new();
    let mut full_response_thinking = String::new();

    // Tool calling loop - may need multiple iterations
    const MAX_TOOL_ITERATIONS: usize = 10;

    for iteration in 0..MAX_TOOL_ITERATIONS {
        if state.is_cancelled.load(Ordering::SeqCst) {
            break;
        }

        // Build request
        let request_body = OpenAIRequest {
            model: "qwen3-vl".to_string(),
            messages: conversation_messages
                .iter()
                .map(|m| serde_json::to_value(m).unwrap())
                .collect(),
            stream: true,
            max_tokens,
            tools: tools_json.clone(),
        };

        let request_builder = client
            .post(format!("{}/v1/chat/completions", state.server_url))
            .header("Content-Type", "application/json")
            .json(&request_body);

        let mut es = EventSource::new(request_builder).map_err(|e| e.to_string())?;

        let mut iteration_content = String::new();
        let mut iteration_thinking = String::new();
        let mut tool_calls: Vec<AccumulatedToolCall> = Vec::new();

        // Stream response
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
                            // Handle content deltas
                            let content_delta = choice.delta.content.clone().unwrap_or_default();
                            let reasoning_delta =
                                choice.delta.reasoning_content.clone().unwrap_or_default();

                            if !content_delta.is_empty() {
                                iteration_content.push_str(&content_delta);
                                full_response_content.push_str(&content_delta);
                            }
                            if !reasoning_delta.is_empty() {
                                iteration_thinking.push_str(&reasoning_delta);
                                full_response_thinking.push_str(&reasoning_delta);
                            }

                            // Emit delta for content (always, even during tool calls for any partial content)
                            if !content_delta.is_empty() || !reasoning_delta.is_empty() {
                                let _ = app.emit(
                                    "chat:delta",
                                    ChatDeltaPayload {
                                        chat_id: chat_id.clone(),
                                        delta: content_delta,
                                        reasoning_delta,
                                    },
                                );
                            }

                            // Handle tool call deltas
                            if let Some(tc_deltas) = &choice.delta.tool_calls {
                                for tc_delta in tc_deltas {
                                    let idx = tc_delta.index;

                                    // Ensure we have enough slots
                                    while tool_calls.len() <= idx {
                                        tool_calls.push(AccumulatedToolCall::default());
                                    }

                                    // Accumulate ID
                                    if let Some(id) = &tc_delta.id {
                                        tool_calls[idx].id = id.clone();
                                    }

                                    // Accumulate function info
                                    if let Some(func) = &tc_delta.function {
                                        if let Some(name) = &func.name {
                                            tool_calls[idx].name = name.clone();
                                        }
                                        if let Some(args) = &func.arguments {
                                            tool_calls[idx].arguments.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[SSE Error] {:?}", e);
                    break;
                }
            }
        }

        // If no tool calls, we're done
        if tool_calls.is_empty() {
            break;
        }

        println!(
            "[tools] Iteration {}: {} tool calls detected",
            iteration,
            tool_calls.len()
        );

        // Build the assistant message with tool calls
        let assistant_tool_calls: Vec<ToolCall> = tool_calls
            .iter()
            .map(|tc| ToolCall {
                id: tc.id.clone(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                },
            })
            .collect();

        conversation_messages.push(ConversationMessage::AssistantWithTools(
            AssistantMessageWithToolCalls {
                role: "assistant".to_string(),
                content: if iteration_content.is_empty() {
                    None
                } else {
                    Some(iteration_content)
                },
                tool_calls: assistant_tool_calls,
            },
        ));

        // Execute each tool call
        for tc in &tool_calls {
            let arguments: Value =
                serde_json::from_str(&tc.arguments).unwrap_or(Value::Object(Default::default()));

            // Emit tool:calling event
            let _ = app.emit(
                "tool:calling",
                ToolCallingPayload {
                    chat_id: chat_id.clone(),
                    tool_id: tc.name.clone(),
                    tool_name: tc.name.clone(),
                    call_id: tc.id.clone(),
                    arguments: arguments.clone(),
                },
            );

            println!("[tools] Executing: {} with args: {}", tc.name, tc.arguments);

            // Execute the tool
            let request = ToolCallRequest {
                tool_id: tc.name.clone(),
                call_id: tc.id.clone(),
                arguments,
            };

            let result = execute_tool(&request).await;

            // Emit tool:result event
            let _ = app.emit(
                "tool:result",
                ToolResultPayload {
                    chat_id: chat_id.clone(),
                    call_id: tc.id.clone(),
                    tool_id: tc.name.clone(),
                    success: result.success,
                    output: result.output.clone(),
                    error: result.error.clone(),
                },
            );

            // Add tool result to conversation
            let result_content = if result.success {
                result.output
            } else {
                format!("Error: {}", result.error.unwrap_or_default())
            };

            conversation_messages.push(ConversationMessage::ToolResult(ToolResultMessage {
                role: "tool".to_string(),
                tool_call_id: tc.id.clone(),
                content: result_content,
            }));
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
