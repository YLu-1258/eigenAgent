// src-tauri/src/types/chat.rs

use serde::{Deserialize, Serialize};

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
pub struct ChatStreamArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    pub chat_id: String,
    pub prompt: String,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Deserialize)]
pub struct RenameChatArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    pub chat_id: String,
    pub title: String,
}

#[derive(Deserialize)]
pub struct DeleteChatArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    pub chat_id: String,
}

#[derive(Deserialize)]
pub struct GenerateTitleArgs {
    #[serde(alias = "chat_id", alias = "chatId")]
    pub chat_id: String,
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
