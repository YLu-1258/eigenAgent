// src/types/tools.ts

export type ToolCategory = "search" | "web" | "filesystem" | "system";

export interface ToolDefinition {
    id: string;
    name: string;
    description: string;
    icon: string;
    category: ToolCategory;
    requiresConfirmation: boolean;
    parameters: Record<string, unknown>;
}

export interface ToolWithStatus extends ToolDefinition {
    enabled: boolean;
}

export interface ToolCallDisplay {
    id: string;
    toolId: string;
    toolName: string;
    arguments: Record<string, unknown>;
    status: "pending" | "running" | "success" | "error";
    output?: string;
    error?: string;
}

// Event payloads from backend
export interface ToolCallingPayload {
    chatId: string;
    toolId: string;
    toolName: string;
    callId: string;
    arguments: Record<string, unknown>;
}

export interface ToolResultPayload {
    chatId: string;
    callId: string;
    toolId: string;
    success: boolean;
    output: string;
    error?: string;
}

// Icon mapping for tools
export const TOOL_ICONS: Record<string, string> = {
    book: "📚",
    globe: "🌐",
    folder: "📁",
    terminal: "💻",
    calculator: "🔢",
};

// Category display names
export const CATEGORY_NAMES: Record<ToolCategory, string> = {
    search: "Search",
    web: "Web",
    filesystem: "File System",
    system: "System",
};
