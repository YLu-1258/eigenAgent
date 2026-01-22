// src/types/chat.ts

export type Role = "user" | "assistant";

export type ImageAttachment = {
    id: string;
    base64: string;
    previewUrl: string;
};

export type FileAttachment = {
    id: string;
    name: string;
    type: "text" | "code" | "document";
    content: string;
    language?: string; // for code files
};

export type ChatMessage = {
    id: string;
    role: Role;
    content: string;
    thinking: string;
    images: ImageAttachment[];
    files: FileAttachment[];
    isStreaming: boolean;
    durationMs?: number;
};

export type ChatHistoryItem = {
    id: string;
    title: string;
    updated_at: number;
    preview: string;
};

export type ChatMessageRow = {
    id: string;
    role: string;
    content: string;
    thinking: string;
    images: string[];
    created_at: number;
    duration_ms?: number;
};

export type ChatBeginPayload = {
    chat_id: string;
};

export type ChatDeltaPayload = {
    chat_id: string;
    delta: string;
    reasoning_delta: string;
};

export type ChatEndPayload = {
    chat_id: string;
    duration_ms: number;
};

export const DRAFT_CHAT_ID = "__draft__";
