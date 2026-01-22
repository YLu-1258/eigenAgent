// src-tauri/src/types/openai.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<Value>,  // Using Value to support different message types (standard, tool calls, tool results)
    pub stream: bool,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
}

#[derive(Serialize, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: OpenAIContent,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum OpenAIContent {
    Text(String),
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
pub enum OpenAIContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlData },
}

#[derive(Serialize, Clone)]
pub struct ImageUrlData {
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct OpenAIStreamResponse {
    pub choices: Vec<OpenAIStreamChoice>,
}

#[derive(Deserialize, Debug)]
pub struct OpenAIStreamChoice {
    pub delta: OpenAIDelta,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAIDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub function: Option<FunctionCallDelta>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct OpenAINonStreamResponse {
    pub choices: Vec<OpenAINonStreamChoice>,
}

#[derive(Deserialize, Debug)]
pub struct OpenAINonStreamChoice {
    pub message: OpenAINonStreamMessage,
}

#[derive(Deserialize, Debug)]
pub struct OpenAINonStreamMessage {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
}

// Tool call structures for building tool messages
#[derive(Serialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// Assistant message with tool calls
#[derive(Serialize, Clone)]
pub struct AssistantMessageWithToolCalls {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

// Tool result message
#[derive(Serialize, Clone)]
pub struct ToolResultMessage {
    pub role: String,
    pub tool_call_id: String,
    pub content: String,
}
