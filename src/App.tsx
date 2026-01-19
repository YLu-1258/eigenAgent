import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import "katex/dist/katex.min.css";
import "./App.css";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";

const DRAFT_CHAT_ID = "__draft__"

type Role = "user" | "assistant";

type ImageAttachment = {
    id: string;
    base64: string;
    previewUrl: string;
};

type ChatMessage = {
    id: string;
    role: Role;
    content: string;
    thinking: string;
    images: ImageAttachment[];
    isStreaming: boolean;
};

type ChatHistoryItem = {
    id: string;
    title: string;
    updated_at: number;
    preview: string;
};

type ChatMessageRow = {
    id: string;
    role: string;
    content: string;
    thinking: string;
    images: string[];
    created_at: number;
};

type ChatBeginPayload = {
    chat_id: string;
};

type ChatDeltaPayload = {
    chat_id: string;
    delta: string;
};

type ChatEndPayload = {
    chat_id: string;
};

function uid() {
    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function welcomeMessage(): ChatMessage {
    return {
        id: uid(),
        role: "assistant",
        content: "Hi — ask me anything.\n\nI can render **Markdown** and see images too!",
        thinking: "",
        images: [],
        isStreaming: false,
    };
}

export default function App() {
    const [messages, setMessages] = useState<ChatMessage[]>([welcomeMessage()]);
    const [chatId, setChatId] = useState<string>(DRAFT_CHAT_ID);
    const [modelReady, setModelReady] = useState(false);
    const [modelError, setModelError] = useState<string | null>(null);
    const [input, setInput] = useState("");
    const [isGenerating, setIsGenerating] = useState(false);
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const [chatHistory, setChatHistory] = useState<ChatHistoryItem[]>([]);
    const [pendingImages, setPendingImages] = useState<ImageAttachment[]>([]);

    const currentAssistantIdRef = useRef<string | null>(null);
    const inThinkRef = useRef(false);
    const fileInputRef = useRef<HTMLInputElement>(null);

    const [selectedThinkingId, setSelectedThinkingId] = useState<string | null>(null);
    const selectedThinkingMsg = useMemo(
        () => messages.find((m) => m.id === selectedThinkingId) ?? null,
        [messages, selectedThinkingId]
    );

    const unlistenBeginRef = useRef<null | (() => void)>(null);
    const unlistenDeltaRef = useRef<null | (() => void)>(null);
    const unlistenEndRef = useRef<null | (() => void)>(null);

    const endRef = useRef<HTMLDivElement | null>(null);
    const activeChatIdRef = useRef(chatId);
    useEffect(() => {
        activeChatIdRef.current = chatId;
    }, [chatId]);

    async function refreshChats() {
        try {
            const chats = await invoke<ChatHistoryItem[]>("list_chats");
            setChatHistory(chats);
        } catch (e) {
            console.log("[list_chats] error", e);
        }
    }

    async function loadChat(chat_id: string) {
        try {
            setChatId(chat_id);

            const rows = await invoke<ChatMessageRow[]>("get_chat_messages", { chatId: chat_id });

            const loaded: ChatMessage[] = rows.map((r) => ({
                id: r.id,
                role: (r.role === "assistant" ? "assistant" : "user") as Role,
                content: r.content,
                thinking: r.thinking || "",
                images: (r.images || []).map((base64) => ({
                    id: uid(),
                    base64,
                    previewUrl: `data:image/jpeg;base64,${base64}`,
                })),
                isStreaming: false,
            }));

            setSelectedThinkingId(null);
            setIsGenerating(false);
            setPendingImages([]);
            currentAssistantIdRef.current = null;
            inThinkRef.current = false;

            setMessages(loaded.length > 0 ? loaded : [welcomeMessage()]);
        } catch (e) {
            console.error("[get_chat_messages] error", e);
        }
    }

    function resetToDraftChat() {
        setChatId(DRAFT_CHAT_ID);
        setMessages([welcomeMessage()]);
        setSelectedThinkingId(null);
        setIsGenerating(false);
        setPendingImages([]);
        currentAssistantIdRef.current = null;
        inThinkRef.current = false;
    }

    // Model loading events
    useEffect(() => {
        let unReady: null | (() => void) = null;
        let unErr: null | (() => void) = null;
        let unLoading: null | (() => void) = null;

        (async () => {
            unLoading = await listen("model:loading", () => {
                console.log("[event] model:loading");
            });

            unReady = await listen("model:ready", () => {
                console.log("[event] model:ready");
                setModelReady(true);
            });

            unErr = await listen<string>("model:error", (e) => {
                console.log("[event] model:error", e.payload);
                setModelError(e.payload);
            });
        })();

        return () => {
            unLoading?.();
            unReady?.();
            unErr?.();
        };
    }, []);

    // Poll model_status so we don't miss ready event
    useEffect(() => {
        let cancelled = false;

        async function poll() {
            try {
                const ready = await invoke<boolean>("model_status");
                if (cancelled) return;

                if (ready) {
                    console.log("[poll] model is ready");
                    setModelReady(true);
                    return;
                }

                setTimeout(poll, 250);
            } catch (e) {
                console.log("[poll] model_status error", e);
                setTimeout(poll, 500);
            }
        }

        poll();
        return () => {
            cancelled = true;
        };
    }, []);

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
    }, []);

    // Chat streaming events
    useEffect(() => {
        let mounted = true;

        (async () => {
            unlistenBeginRef.current = await listen<ChatBeginPayload>("chat:begin", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                setIsGenerating(true);
                inThinkRef.current = false;

                const assistantId = uid();
                currentAssistantIdRef.current = assistantId;

                setMessages((prev) => [
                    ...prev,
                    { id: assistantId, role: "assistant", content: "", thinking: "", images: [], isStreaming: true },
                ]);

                setSelectedThinkingId(assistantId);
            });

            unlistenDeltaRef.current = await listen<ChatDeltaPayload>("chat:delta", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                const delta = event.payload.delta ?? "";
                const assistantId = currentAssistantIdRef.current;
                if (!assistantId) return;

                setMessages((prev) =>
                    prev.map((m) => {
                        if (m.id !== assistantId) return m;

                        let thinking = m.thinking;
                        let content = m.content;

                        let i = 0;
                        while (i < delta.length) {
                            if (!inThinkRef.current) {
                                const start = delta.indexOf("<think>", i);
                                if (start === -1) {
                                    content += delta.slice(i);
                                    break;
                                }
                                content += delta.slice(i, start);
                                inThinkRef.current = true;
                                i = start + "<think>".length;
                            } else {
                                const end = delta.indexOf("</think>", i);
                                if (end === -1) {
                                    thinking += delta.slice(i);
                                    break;
                                }
                                thinking += delta.slice(i, end);
                                inThinkRef.current = false;
                                i = end + "</think>".length;
                            }
                        }

                        content = content.replaceAll("</think>", "").replaceAll("<think>", "");
                        thinking = thinking.replaceAll("</think>", "").replaceAll("<think>", "");

                        return { ...m, thinking, content };
                    })
                );
            });

            unlistenEndRef.current = await listen<ChatEndPayload>("chat:end", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                setIsGenerating(false);
                inThinkRef.current = false;

                const assistantId = currentAssistantIdRef.current;
                currentAssistantIdRef.current = null;

                if (assistantId) {
                    setMessages((prev) =>
                        prev.map((m) => (m.id === assistantId ? { ...m, isStreaming: false } : m))
                    );
                }

                refreshChats();
            });
        })();

        return () => {
            mounted = false;
            unlistenBeginRef.current?.();
            unlistenDeltaRef.current?.();
            unlistenEndRef.current?.();
            unlistenBeginRef.current = null;
            unlistenDeltaRef.current = null;
            unlistenEndRef.current = null;
        };
    }, [chatId]);

    const canSend = useMemo(
        () => (input.trim().length > 0 || pendingImages.length > 0) && !isGenerating && modelReady,
        [input, pendingImages, isGenerating, modelReady]
    );

    async function handleImageSelect(e: React.ChangeEvent<HTMLInputElement>) {
        const files = e.target.files;
        if (!files) return;

        for (const file of Array.from(files)) {
            if (!file.type.startsWith("image/")) continue;
            if (file.size > 10 * 1024 * 1024) {
                console.warn("Image too large, skipping:", file.name);
                continue;
            }

            const base64 = await fileToBase64(file);
            setPendingImages((prev) => [
                ...prev,
                {
                    id: uid(),
                    base64,
                    previewUrl: `data:${file.type};base64,${base64}`,
                },
            ]);
        }

        // Reset file input
        if (fileInputRef.current) {
            fileInputRef.current.value = "";
        }
    }

    function fileToBase64(file: File): Promise<string> {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
                const result = reader.result as string;
                // Remove data URL prefix
                resolve(result.split(",")[1]);
            };
            reader.onerror = reject;
            reader.readAsDataURL(file);
        });
    }

    function removePendingImage(id: string) {
        setPendingImages((prev) => prev.filter((img) => img.id !== id));
    }

    async function handleSend() {
        const text = input.trim();
        if ((!text && pendingImages.length === 0) || isGenerating || !modelReady) return;

        const userImages = [...pendingImages];

        setMessages((prev) => [
            ...prev,
            { id: uid(), role: "user", content: text, thinking: "", images: userImages, isStreaming: false },
        ]);
        setInput("");
        setPendingImages([]);

        try {
            setIsGenerating(true);

            let chat_id = chatId;

            if (chat_id === DRAFT_CHAT_ID) {
                chat_id = await invoke<string>("new_chat");
                setChatId(chat_id);
                await refreshChats();
            }

            await invoke("chat_stream", {
                args: {
                    chatId: chat_id,
                    prompt: text,
                    images: userImages.map((img) => img.base64),
                },
            });
        } catch (err) {
            setIsGenerating(false);
            inThinkRef.current = false;
            currentAssistantIdRef.current = null;

            setMessages((prev) => [
                ...prev,
                {
                    id: uid(),
                    role: "assistant",
                    content: `Error: ${String(err)}`,
                    thinking: "",
                    images: [],
                    isStreaming: false,
                },
            ]);
        }
    }

    function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === "Enter") {
            e.preventDefault();
            handleSend();
        }
    }

    async function handleStop() {
        try {
            await invoke("cancel_generation");
        } catch (err) {
            console.error("[cancel_generation] error", err);
        }
    }

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

    if (!modelReady) {
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

    return (
        <div className="app">
            {/* LEFT SIDEBAR */}
            <div className={`sidebar ${sidebarOpen ? "open" : "closed"}`}>
                <div className="sidebarHeader">
                    <button className="newChatBtn" onClick={() => resetToDraftChat()}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M12 5v14M5 12h14" />
                        </svg>
                        New chat
                    </button>
                </div>

                <div className="historySection niceScroll">
                    <div className="historyLabel">Recent</div>

                    {chatHistory.map((chat) => (
                        <div
                            key={chat.id}
                            className={`historyItem ${chatId === chat.id ? "active" : ""}`}
                            onClick={() => {
                                if (chatId !== chat.id) loadChat(chat.id);
                            }}
                            style={{ cursor: "pointer" }}
                            title={chat.preview}
                        >
                            <div className="historyTitle">{chat.title}</div>
                            <div className="historyPreview">{chat.preview}</div>
                            <div className="historyTime">
                                {new Date(chat.updated_at).toLocaleDateString(undefined, { month: "short", day: "numeric" })}
                            </div>
                        </div>
                    ))}
                </div>

                <div className="sidebarFooter">
                    <div className="userSection">
                        <div className="userAvatar">E</div>
                        <div className="userName">Eigen</div>
                    </div>
                </div>
            </div>

            {/* TOGGLE SIDEBAR BUTTON */}
            <button
                className="sidebarToggle"
                onClick={() => setSidebarOpen(!sidebarOpen)}
                title={sidebarOpen ? "Close sidebar" : "Open sidebar"}
            >
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    {sidebarOpen ? <path d="M15 18l-6-6 6-6" /> : <path d="M9 18l6-6-6-6" />}
                </svg>
            </button>

            {/* CENTER: Chat */}
            <div className="chatCol">
                <div className="chatHeader">
                    <div className="chatTitle">Eigen</div>
                    <div className="statusIndicator">
                        <div className="statusDot"></div>
                        Online
                    </div>
                </div>

                <div className="chatScroll niceScroll">
                    {messages.map((m) => {
                        const isUser = m.role === "user";
                        const showPlaceholder = !isUser && m.isStreaming && m.content.trim().length === 0;

                        return (
                            <div key={m.id} className={`msgRow ${isUser ? "right" : "left"}`}>
                                {!isUser && (
                                    <div className="msgAvatar">
                                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                                            <circle cx="12" cy="12" r="10" fill="url(#gradient)" />
                                            <defs>
                                                <linearGradient id="gradient" x1="0%" y1="0%" x2="100%" y2="100%">
                                                    <stop offset="0%" stopColor="#3b82f6" />
                                                    <stop offset="100%" stopColor="#2563eb" />
                                                </linearGradient>
                                            </defs>
                                        </svg>
                                    </div>
                                )}

                                <div className="msgStack">
                                    {/* Display images if present */}
                                    {m.images && m.images.length > 0 && (
                                        <div className="messageImages">
                                            {m.images.map((img) => (
                                                <img
                                                    key={img.id}
                                                    src={img.previewUrl}
                                                    alt="attachment"
                                                    className="messageImage"
                                                />
                                            ))}
                                        </div>
                                    )}

                                    <div
                                        className={`bubble ${isUser ? "userBubble" : "assistantBubble"} ${
                                            !isUser && m.id === selectedThinkingId ? "selected" : ""
                                        }`}
                                        title={!isUser ? "Click to view thinking" : undefined}
                                        onClick={() => {
                                            if (!isUser) setSelectedThinkingId(m.id);
                                        }}
                                    >
                                        {showPlaceholder ? (
                                            <div className="thinkingIndicator">
                                                <span className="dot"></span>
                                                <span className="dot"></span>
                                                <span className="dot"></span>
                                            </div>
                                        ) : !isUser ? (
                                            <div className="md">
                                                <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKatex]}>
                                                    {m.content}
                                                </ReactMarkdown>
                                            </div>
                                        ) : (
                                            <span className="userText">{m.content}</span>
                                        )}
                                    </div>

                                    {!isUser && (
                                        <button className="thinkBtn" onClick={() => setSelectedThinkingId(m.id)}>
                                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                <circle cx="12" cy="12" r="3" />
                                                <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
                                            </svg>
                                            View thinking
                                        </button>
                                    )}
                                </div>

                                {isUser && <div className="msgAvatar user">U</div>}
                            </div>
                        );
                    })}
                    <div ref={endRef} />
                </div>

                <div className="inputRow">
                    {/* Pending images preview */}
                    {pendingImages.length > 0 && (
                        <div className="pendingImages">
                            {pendingImages.map((img) => (
                                <div key={img.id} className="pendingImageThumb">
                                    <img src={img.previewUrl} alt="pending" />
                                    <button onClick={() => removePendingImage(img.id)} title="Remove image">
                                        &times;
                                    </button>
                                </div>
                            ))}
                        </div>
                    )}

                    <div className="inputContainer">
                        <button
                            className="imageUploadBtn"
                            onClick={() => fileInputRef.current?.click()}
                            disabled={isGenerating}
                            title="Upload image"
                        >
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                                <circle cx="8.5" cy="8.5" r="1.5" />
                                <polyline points="21 15 16 10 5 21" />
                            </svg>
                        </button>
                        <input
                            type="file"
                            ref={fileInputRef}
                            accept="image/*"
                            multiple
                            onChange={handleImageSelect}
                            style={{ display: "none" }}
                        />
                        <input
                            value={input}
                            onChange={(e) => setInput(e.target.value)}
                            onKeyDown={onKeyDown}
                            placeholder="Ask anything..."
                            className="input"
                            disabled={isGenerating}
                        />
                        {isGenerating ? (
                            <button className="stopBtn" onClick={handleStop} title="Stop generating">
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                                    <rect x="6" y="6" width="12" height="12" rx="2" />
                                </svg>
                            </button>
                        ) : (
                            <button className={`sendBtn ${canSend ? "active" : ""}`} onClick={handleSend} disabled={!canSend}>
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z" />
                                </svg>
                            </button>
                        )}
                    </div>
                </div>
            </div>

            {/* RIGHT: Thinking panel */}
            <div className="thinkingCol">
                <div className="thinkingHeader">
                    <div className="thinkingTitle">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                        </svg>
                        Thinking Process
                    </div>
                    <div className="thinkingStatus">
                        {selectedThinkingMsg?.isStreaming && <span className="streamingBadge">Live</span>}
                    </div>
                </div>

                {!selectedThinkingMsg ? (
                    <div className="thinkingEmpty">
                        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                            <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                        </svg>
                        <div>Click on any assistant message to see its thinking process</div>
                    </div>
                ) : (
                    <div className="thinkingBox niceScroll">
                        {selectedThinkingMsg.thinking.trim().length > 0
                            ? selectedThinkingMsg.thinking
                            : "No thinking captured for this message."}
                    </div>
                )}
            </div>
        </div>
    );
}
