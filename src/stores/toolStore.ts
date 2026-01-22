// src/stores/toolStore.ts

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import {
    ToolWithStatus,
    ToolCallDisplay,
    ToolCallingPayload,
    ToolResultPayload,
} from "../types/tools";

interface ToolState {
    tools: ToolWithStatus[];
    isLoading: boolean;
    error: string | null;

    // Active tool calls (for display in chat)
    activeToolCalls: Map<string, ToolCallDisplay>;

    // Actions
    loadTools: () => Promise<void>;
    toggleTool: (toolId: string, enabled: boolean) => Promise<void>;

    // Tool call tracking
    addToolCall: (payload: ToolCallingPayload) => void;
    updateToolResult: (payload: ToolResultPayload) => void;
    clearToolCalls: () => void;
    getToolCallsForChat: (chatId: string) => ToolCallDisplay[];
}

export const useToolStore = create<ToolState>((set, get) => ({
    tools: [],
    isLoading: false,
    error: null,
    activeToolCalls: new Map(),

    loadTools: async () => {
        set({ isLoading: true, error: null });
        try {
            const tools = await invoke<ToolWithStatus[]>("cmd_list_tools");
            set({ tools, isLoading: false });
        } catch (e) {
            console.error("[tools] Failed to load:", e);
            set({ error: String(e), isLoading: false });
        }
    },

    toggleTool: async (toolId: string, enabled: boolean) => {
        try {
            await invoke("cmd_toggle_tool", { toolId, enabled });
            // Update local state
            const { tools } = get();
            const updatedTools = tools.map((t) =>
                t.id === toolId ? { ...t, enabled } : t
            );
            set({ tools: updatedTools });
        } catch (e) {
            console.error("[tools] Failed to toggle:", e);
            set({ error: String(e) });
        }
    },

    addToolCall: (payload: ToolCallingPayload) => {
        const { activeToolCalls } = get();
        const newCall: ToolCallDisplay = {
            id: payload.callId,
            toolId: payload.toolId,
            toolName: payload.toolName,
            arguments: payload.arguments,
            status: "running",
        };
        const newMap = new Map(activeToolCalls);
        newMap.set(payload.callId, newCall);
        set({ activeToolCalls: newMap });
    },

    updateToolResult: (payload: ToolResultPayload) => {
        const { activeToolCalls } = get();
        const newMap = new Map(activeToolCalls);
        const existing = newMap.get(payload.callId);
        if (existing) {
            newMap.set(payload.callId, {
                ...existing,
                status: payload.success ? "success" : "error",
                output: payload.output,
                error: payload.error,
            });
            set({ activeToolCalls: newMap });
        }
    },

    clearToolCalls: () => {
        set({ activeToolCalls: new Map() });
    },

    getToolCallsForChat: (_chatId: string) => {
        // For now, return all active tool calls
        // In a more sophisticated implementation, we'd track by chatId
        const { activeToolCalls } = get();
        return Array.from(activeToolCalls.values());
    },
}));
