// src/App.tsx

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import "katex/dist/katex.min.css";
import "./App.css";

// Types
import { ChatBeginPayload, ChatDeltaPayload, ChatEndPayload, DRAFT_CHAT_ID } from "./types/chat";

// Stores
import { useSettingsStore } from "./stores/settingsStore";
import { useChatStore } from "./stores/chatStore";

// Hooks
import { useModels } from "./hooks/useModels";
import { useFileUpload } from "./hooks/useFileUpload";

// Utils
import { uid } from "./utils/format";

// Components
import { SettingsModal } from "./components/SettingsModal";
import { Sidebar } from "./components/Sidebar";
import { Chat } from "./components/Chat";
import { ThinkingPanel } from "./components/ThinkingPanel";

export default function App() {
    const [input, setInput] = useState("");
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const [settingsOpen, setSettingsOpen] = useState(false);

    // Chat store
    const {
        messages,
        chatId,
        isGenerating,
        chatHistory,
        selectedThinkingId,
        setMessages,
        setChatId,
        setIsGenerating,
        setSelectedThinkingId,
        setCurrentAssistantId,
        setPendingTitleChatId,
        loadChat,
        resetToDraftChat,
        refreshChats,
        deleteChat,
        beginStreaming,
        appendDelta,
        endStreaming,
        handleStop,
    } = useChatStore();

    // Model hook
    const {
        models,
        currentModelName,
        modelReady,
        modelError,
        modelSwitching,
        noModelInstalled,
        initialCheckDone,
        downloadProgress,
        switchModel,
        downloadModel,
        cancelDownload,
        deleteModel,
    } = useModels(isGenerating);

    // File upload hook
    const {
        pendingImages,
        pendingFiles,
        fileInputRef,
        handleFileSelect,
        removePendingImage,
        removePendingFile,
        clearPending,
        consumePending,
    } = useFileUpload();

    // Settings store
    const { settings, loadSettings } = useSettingsStore();

    // Refs
    const activeChatIdRef = useRef(chatId);
    useEffect(() => {
        activeChatIdRef.current = chatId;
    }, [chatId]);

    // Selected thinking message
    const selectedThinkingMsg = useMemo(
        () => messages.find((m) => m.id === selectedThinkingId) ?? null,
        [messages, selectedThinkingId]
    );

    // Can send check
    const canSend = useMemo(
        () => (input.trim().length > 0 || pendingImages.length > 0 || pendingFiles.length > 0) && !isGenerating && modelReady && !modelSwitching,
        [input, pendingImages, pendingFiles, isGenerating, modelReady, modelSwitching]
    );

    // Chat streaming events
    useEffect(() => {
        let mounted = true;
        let unlistenBegin: null | (() => void) = null;
        let unlistenDelta: null | (() => void) = null;
        let unlistenEnd: null | (() => void) = null;

        (async () => {
            unlistenBegin = await listen<ChatBeginPayload>("chat:begin", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                beginStreaming(event.payload.chat_id);
            });

            unlistenDelta = await listen<ChatDeltaPayload>("chat:delta", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                const content_delta = event.payload.delta ?? "";
                const reasoning_delta = event.payload.reasoning_delta ?? "";
                appendDelta(content_delta, reasoning_delta);
            });

            unlistenEnd = await listen<ChatEndPayload>("chat:end", (event) => {
                if (!mounted) return;
                if (!event.payload) return;

                const eventChatId = event.payload.chat_id;
                const state = useChatStore.getState();

                // Only process streaming end for the active chat
                if (eventChatId === activeChatIdRef.current) {
                    endStreaming(event.payload.duration_ms);
                }

                // Generate title if this chat was pending title generation
                // This works even if user switched to a different chat
                if (state.pendingTitleChatId === eventChatId) {
                    setPendingTitleChatId(null);
                    console.log("[generate_chat_title] requesting title for:", eventChatId);
                    invoke("generate_chat_title", { args: { chatId: eventChatId } })
                        .then(() => {
                            console.log("[generate_chat_title] completed for:", eventChatId);
                            refreshChats(); // Refresh to show new title
                        })
                        .catch((err) => console.error("[generate_chat_title] error:", err));
                }

                refreshChats();
            });
        })();

        return () => {
            mounted = false;
            unlistenBegin?.();
            unlistenDelta?.();
            unlistenEnd?.();
        };
    }, [chatId, beginStreaming, appendDelta, endStreaming, setPendingTitleChatId, refreshChats]);

    // Load chat list initially + whenever backend says it changed
    useEffect(() => {
        refreshChats();

        let un: null | (() => void) = null;
        (async () => {
            un = await listen("chats:changed", () => {
                refreshChats();
            });
        })();

        return () => {
            un?.();
        };
    }, [refreshChats]);

    // Load settings on mount
    useEffect(() => {
        loadSettings();
    }, [loadSettings]);

    // Keyboard shortcut for settings (Cmd/Ctrl + ,)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if ((e.metaKey || e.ctrlKey) && e.key === ",") {
                e.preventDefault();
                setSettingsOpen(true);
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, []);

    // Handle send message
    async function handleSend() {
        const text = input.trim();
        if ((!text && pendingImages.length === 0 && pendingFiles.length === 0) || isGenerating || !modelReady) return;

        const { images: userImages, files: userFiles } = consumePending();

        // Build the full prompt with file contents
        let fullPrompt = text;

        if (userFiles.length > 0) {
            const fileContents = userFiles.map((f) => {
                const lang = f.language || f.type;
                return `\n\n--- File: ${f.name} ---\n\`\`\`${lang}\n${f.content}\n\`\`\``;
            }).join("");

            fullPrompt = text + fileContents;
        }

        setMessages((prev) => [
            ...prev,
            { id: uid(), role: "user", content: text, thinking: "", images: userImages, files: userFiles, isStreaming: false },
        ]);
        setInput("");

        try {
            setIsGenerating(true);

            let chat_id = chatId;

            if (chat_id === DRAFT_CHAT_ID) {
                chat_id = await invoke<string>("new_chat");
                setChatId(chat_id);
                setPendingTitleChatId(chat_id); // Mark this specific chat for title generation
                await refreshChats();
            }

            await invoke("chat_stream", {
                args: {
                    chatId: chat_id,
                    prompt: fullPrompt,
                    images: userImages.map((img) => img.base64),
                },
            });
        } catch (err) {
            setIsGenerating(false);
            setCurrentAssistantId(null);

            setMessages((prev) => [
                ...prev,
                {
                    id: uid(),
                    role: "assistant",
                    content: `Error: ${String(err)}`,
                    thinking: "",
                    images: [],
                    files: [],
                    isStreaming: false,
                },
            ]);
        }
    }

    function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === "Enter") {
            // If sendOnEnter is enabled, Enter sends, Shift+Enter is newline
            // If sendOnEnter is disabled, Shift+Enter sends, Enter is newline
            if (settings.behavior.sendOnEnter) {
                if (!e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                }
            } else {
                if (e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                }
            }
        }
    }

    // Handle chat deletion
    function handleDeleteChat(chatId: string, e: React.MouseEvent) {
        e.stopPropagation();
        deleteChat(chatId);
    }

    // Handle new chat + clear pending files
    function handleNewChat() {
        resetToDraftChat();
        clearPending();
    }

    // Error screen
    if (modelError) {
        return (
            <div className="screen">
                <div className="errorCard">
                    <h2>Failed to load model</h2>
                    <pre>{modelError}</pre>
                </div>
            </div>
        );
    }

    // Loading screen when model is loading
    if (!modelReady && initialCheckDone && !noModelInstalled) {
        return (
            <div className="screen">
                <div className="loadingCard">
                    <div className="loadingSpinner"></div>
                    <div>Starting llama-server...</div>
                    <div className="loadingSubtext">This may take a moment on first launch</div>
                </div>
            </div>
        );
    }

    // Brief loading state while checking for models
    if (!initialCheckDone) {
        return (
            <div className="screen">
                <div className="loadingCard">
                    <div className="loadingSpinner"></div>
                    <div>Loading...</div>
                </div>
            </div>
        );
    }

    return (
        <div className="app">
            {/* LEFT SIDEBAR */}
            <Sidebar
                isOpen={sidebarOpen}
                chatHistory={chatHistory}
                currentChatId={chatId}
                models={models}
                currentModelName={currentModelName}
                noModelInstalled={noModelInstalled}
                modelSwitching={modelSwitching}
                downloadProgress={downloadProgress}
                onToggle={() => setSidebarOpen(!sidebarOpen)}
                onNewChat={handleNewChat}
                onLoadChat={loadChat}
                onDeleteChat={handleDeleteChat}
                onSwitchModel={switchModel}
                onDownloadModel={downloadModel}
                onCancelDownload={cancelDownload}
                onDeleteModel={deleteModel}
                onOpenSettings={() => setSettingsOpen(true)}
            />

            {/* CENTER: Chat */}
            <Chat
                messages={messages}
                selectedThinkingId={selectedThinkingId}
                noModelInstalled={noModelInstalled}
                input={input}
                pendingImages={pendingImages}
                pendingFiles={pendingFiles}
                isGenerating={isGenerating}
                canSend={canSend}
                fileInputRef={fileInputRef}
                onSelectThinking={setSelectedThinkingId}
                onInputChange={setInput}
                onKeyDown={onKeyDown}
                onFileSelect={handleFileSelect}
                onRemovePendingImage={removePendingImage}
                onRemovePendingFile={removePendingFile}
                onSend={handleSend}
                onStop={handleStop}
            />

            {/* RIGHT: Thinking panel */}
            <ThinkingPanel selectedMessage={selectedThinkingMsg} />

            {/* Settings Modal */}
            <SettingsModal
                isOpen={settingsOpen}
                onClose={() => setSettingsOpen(false)}
                models={models.filter(m => m.download_status === "downloaded").map(m => ({ id: m.id, name: m.name }))}
            />
        </div>
    );
}
