// src/stores/chatStore.ts

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import {
    ChatMessage,
    ChatHistoryItem,
    ChatMessageRow,
    DRAFT_CHAT_ID,
    ImageAttachment,
} from "../types/chat";
import { uid } from "../utils/format";

function welcomeMessage(): ChatMessage {
    return {
        id: uid(),
        role: "assistant",
        content: "Hi — ask me anything.\n\nI can render **Markdown**, see images, and read files too!",
        thinking: "",
        images: [],
        files: [],
        isStreaming: false,
    };
}

interface ChatState {
    // State
    messages: ChatMessage[];
    chatId: string;
    isGenerating: boolean;
    chatHistory: ChatHistoryItem[];
    selectedThinkingId: string | null;

    // Refs tracked in state (for cross-component access)
    currentAssistantId: string | null;
    needsTitleGeneration: boolean;

    // Actions
    setMessages: (messages: ChatMessage[] | ((prev: ChatMessage[]) => ChatMessage[])) => void;
    setChatId: (chatId: string) => void;
    setIsGenerating: (isGenerating: boolean) => void;
    setSelectedThinkingId: (id: string | null) => void;
    setCurrentAssistantId: (id: string | null) => void;
    setNeedsTitleGeneration: (needs: boolean) => void;

    // Complex actions
    loadChat: (chatId: string) => Promise<void>;
    resetToDraftChat: () => void;
    refreshChats: () => Promise<void>;
    deleteChat: (chatId: string) => Promise<void>;

    // Streaming helpers
    beginStreaming: (chatId: string) => void;
    appendDelta: (contentDelta: string, reasoningDelta: string) => void;
    endStreaming: (durationMs: number) => void;

    // Send/Stop
    handleStop: () => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
    // Initial state
    messages: [welcomeMessage()],
    chatId: DRAFT_CHAT_ID,
    isGenerating: false,
    chatHistory: [],
    selectedThinkingId: null,
    currentAssistantId: null,
    needsTitleGeneration: false,

    // Simple setters
    setMessages: (messagesOrFn) => {
        if (typeof messagesOrFn === "function") {
            set((state) => ({ messages: messagesOrFn(state.messages) }));
        } else {
            set({ messages: messagesOrFn });
        }
    },

    setChatId: (chatId) => set({ chatId }),
    setIsGenerating: (isGenerating) => set({ isGenerating }),
    setSelectedThinkingId: (selectedThinkingId) => set({ selectedThinkingId }),
    setCurrentAssistantId: (currentAssistantId) => set({ currentAssistantId }),
    setNeedsTitleGeneration: (needsTitleGeneration) => set({ needsTitleGeneration }),

    // Complex actions
    loadChat: async (chatId: string) => {
        try {
            set({ chatId });

            const rows = await invoke<ChatMessageRow[]>("get_chat_messages", { chatId });

            const loaded: ChatMessage[] = rows.map((r) => ({
                id: r.id,
                role: (r.role === "assistant" ? "assistant" : "user") as "user" | "assistant",
                content: r.content,
                thinking: r.thinking || "",
                images: (r.images || []).map((base64) => ({
                    id: uid(),
                    base64,
                    previewUrl: `data:image/jpeg;base64,${base64}`,
                })),
                files: [],
                isStreaming: false,
                durationMs: r.duration_ms,
            }));

            set({
                messages: loaded.length > 0 ? loaded : [welcomeMessage()],
                selectedThinkingId: null,
                isGenerating: false,
                currentAssistantId: null,
            });
        } catch (e) {
            console.error("[get_chat_messages] error", e);
        }
    },

    resetToDraftChat: () => {
        set({
            chatId: DRAFT_CHAT_ID,
            messages: [welcomeMessage()],
            selectedThinkingId: null,
            isGenerating: false,
            currentAssistantId: null,
            needsTitleGeneration: false,
        });
    },

    refreshChats: async () => {
        try {
            const chats = await invoke<ChatHistoryItem[]>("list_chats");
            console.log("[list_chats] received:", chats.map(c => ({ id: c.id.slice(0, 8), title: c.title })));
            set({ chatHistory: chats });
        } catch (e) {
            console.log("[list_chats] error", e);
        }
    },

    deleteChat: async (chatId: string) => {
        try {
            await invoke("delete_chat", { args: { chatId } });
            // If the deleted chat is the current one, reset to draft
            if (get().chatId === chatId) {
                get().resetToDraftChat();
            }
            // Chat list will auto-refresh via chats:changed event
        } catch (err) {
            console.error("[delete_chat] error", err);
        }
    },

    // Streaming helpers
    beginStreaming: (chatId: string) => {
        const state = get();
        if (chatId !== state.chatId) return;

        const assistantId = uid();

        set({
            isGenerating: true,
            currentAssistantId: assistantId,
            selectedThinkingId: assistantId,
            messages: [
                ...state.messages,
                {
                    id: assistantId,
                    role: "assistant",
                    content: "",
                    thinking: "",
                    images: [],
                    files: [],
                    isStreaming: true,
                },
            ],
        });
    },

    appendDelta: (contentDelta: string, reasoningDelta: string) => {
        const assistantId = get().currentAssistantId;
        if (!assistantId) return;

        set((state) => ({
            messages: state.messages.map((m) => {
                if (m.id !== assistantId) return m;
                return {
                    ...m,
                    content: m.content + contentDelta,
                    thinking: m.thinking + reasoningDelta,
                };
            }),
        }));
    },

    endStreaming: (durationMs: number) => {
        const assistantId = get().currentAssistantId;

        set((state) => ({
            isGenerating: false,
            currentAssistantId: null,
            messages: assistantId
                ? state.messages.map((m) =>
                      m.id === assistantId
                          ? { ...m, isStreaming: false, durationMs }
                          : m
                  )
                : state.messages,
        }));
    },

    handleStop: async () => {
        try {
            await invoke("cancel_generation");
        } catch (err) {
            console.error("[cancel_generation] error", err);
        }
    },
}));
