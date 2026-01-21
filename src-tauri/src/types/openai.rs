// src-tauri/src/types/openai.rs

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    pub stream: bool,
    pub max_tokens: u32,
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

#[derive(Deserialize, Debug)]
pub struct OpenAIDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
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
