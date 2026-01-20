import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import "katex/dist/katex.min.css";
import "./App.css";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";

// Document parsing libraries
import * as pdfjsLib from "pdfjs-dist";
import pdfjsWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import mammoth from "mammoth";
import * as XLSX from "xlsx";

// Set up PDF.js worker using local bundle (works in Tauri without network)
pdfjsLib.GlobalWorkerOptions.workerSrc = pdfjsWorker;

const DRAFT_CHAT_ID = "__draft__"

type Role = "user" | "assistant";

type ImageAttachment = {
    id: string;
    base64: string;
    previewUrl: string;
};

type FileAttachment = {
    id: string;
    name: string;
    type: "text" | "code" | "document";
    content: string;
    language?: string; // for code files
};

type ChatMessage = {
    id: string;
    role: Role;
    content: string;
    thinking: string;
    images: ImageAttachment[];
    files: FileAttachment[];
    isStreaming: boolean;
    durationMs?: number;
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
    duration_ms?: number;
};

type ChatBeginPayload = {
    chat_id: string;
};

type ChatDeltaPayload = {
    chat_id: string;
    delta: string;
    reasoning_delta: string;
};

type ChatEndPayload = {
    chat_id: string;
    duration_ms: number;
};

// Model Catalog Types
type ModelCapabilities = {
    vision: boolean;
    thinking: boolean;
};

type ModelInfo = {
    id: string;
    name: string;
    description: string;
    size_label: string;
    capabilities: ModelCapabilities;
    download_status: string; // "not_downloaded" | "downloading" | "downloaded"
    download_percent: number | null;
    is_current: boolean;
};

type DownloadProgressPayload = {
    model_id: string;
    downloaded_bytes: number;
    total_bytes: number;
    percent: number;
    speed_bps: number;
};

type ModelSwitchPayload = {
    model_id: string;
    status: string; // "stopping" | "starting" | "ready" | "error"
    error?: string;
};

function uid() {
    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function formatTimestamp(timestamp: number): string {
    const now = new Date();
    const date = new Date(timestamp);
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    const timeStr = date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });

    // Less than 1 minute
    if (diffMins < 1) {
        return "Just now";
    }
    // Less than 1 hour
    if (diffMins < 60) {
        return `${diffMins} min ago`;
    }
    // Less than 24 hours
    if (diffHours < 24) {
        return `${diffHours} ${diffHours === 1 ? "hour" : "hours"} ago`;
    }
    // Yesterday
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (date.toDateString() === yesterday.toDateString()) {
        return `Yesterday at ${timeStr}`;
    }
    // Within last 7 days
    if (diffDays < 7) {
        const dayName = date.toLocaleDateString(undefined, { weekday: "long" });
        return `${dayName} at ${timeStr}`;
    }
    // Older - show date with time
    const dateStr = date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
    return `${dateStr} at ${timeStr}`;
}

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

export default function App() {
    const [messages, setMessages] = useState<ChatMessage[]>([welcomeMessage()]);
    const [chatId, setChatId] = useState<string>(DRAFT_CHAT_ID);
    const [modelReady, setModelReady] = useState(false);
    const [modelError, setModelError] = useState<string | null>(null);
    const [noModelInstalled, setNoModelInstalled] = useState(false);
    const [initialCheckDone, setInitialCheckDone] = useState(false);
    const [input, setInput] = useState("");
    const [isGenerating, setIsGenerating] = useState(false);
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const [chatHistory, setChatHistory] = useState<ChatHistoryItem[]>([]);
    const [pendingImages, setPendingImages] = useState<ImageAttachment[]>([]);
    const [pendingFiles, setPendingFiles] = useState<FileAttachment[]>([]);

    // Model catalog state
    const [models, setModels] = useState<ModelInfo[]>([]);
    const [currentModelId, setCurrentModelId] = useState<string | null>(null);
    const [modelCatalogOpen, setModelCatalogOpen] = useState(false);
    const [modelSwitching, setModelSwitching] = useState(false);
    const [downloadProgress, setDownloadProgress] = useState<Record<string, { percent: number; speed: number }>>({});

    const currentAssistantIdRef = useRef<string | null>(null);
    const inThinkRef = useRef(false);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const needsTitleGenerationRef = useRef(false);

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
            console.log("[list_chats] received:", chats.map(c => ({ id: c.id.slice(0, 8), title: c.title })));
            setChatHistory(chats);
        } catch (e) {
            console.log("[list_chats] error", e);
        }
    }

    // Model catalog functions
    async function refreshModels() {
        try {
            const modelList = await invoke<ModelInfo[]>("list_models");
            setModels(modelList);

            // Find current model
            const current = modelList.find((m) => m.is_current);
            if (current) {
                setCurrentModelId(current.id);
            }
        } catch (e) {
            console.log("[list_models] error", e);
        }
    }

    async function handleSwitchModel(modelId: string) {
        if (modelSwitching || isGenerating) return;

        try {
            setModelSwitching(true);
            setModelReady(false);
            await invoke("switch_model", { args: { modelId } });
        } catch (e) {
            console.error("[switch_model] error", e);
            setModelSwitching(false);
        }
    }

    async function handleDownloadModel(modelId: string) {
        try {
            await invoke("download_model", { args: { modelId } });
        } catch (e) {
            console.error("[download_model] error", e);
        }
    }

    async function handleCancelDownload(modelId: string) {
        try {
            await invoke("cancel_download", { args: { modelId } });
            // Remove from local progress tracking
            setDownloadProgress((prev) => {
                const next = { ...prev };
                delete next[modelId];
                return next;
            });
        } catch (e) {
            console.error("[cancel_download] error", e);
        }
    }

    async function handleDeleteModel(modelId: string) {
        try {
            await invoke("delete_model", { args: { modelId } });
            await refreshModels();
        } catch (e) {
            console.error("[delete_model] error", e);
        }
    }

    function formatBytes(bytes: number): string {
        if (bytes === 0) return "0 B";
        const k = 1024;
        const sizes = ["B", "KB", "MB", "GB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
    }

    function formatSpeed(bps: number): string {
        return formatBytes(bps) + "/s";
    }

    // Computed value for current model name
    const currentModelName = useMemo(() => {
        if (noModelInstalled || !currentModelId) return "No model active!";
        const model = models.find((m) => m.id === currentModelId);
        return model?.name || currentModelId;
    }, [currentModelId, models, noModelInstalled]);

    async function handleDeleteChat(chat_id: string, e: React.MouseEvent) {
        e.stopPropagation(); // Prevent triggering the chat load
        try {
            await invoke("delete_chat", { args: { chatId: chat_id } });
            // If the deleted chat is the current one, reset to draft
            if (chatId === chat_id) {
                resetToDraftChat();
            }
            // Chat list will auto-refresh via chats:changed event
        } catch (err) {
            console.error("[delete_chat] error", err);
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
                files: [],
                isStreaming: false,
                durationMs: r.duration_ms,
            }));

            setSelectedThinkingId(null);
            setIsGenerating(false);
            setPendingImages([]);
            setPendingFiles([]);
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
        setPendingFiles([]);
        currentAssistantIdRef.current = null;
        inThinkRef.current = false;
        needsTitleGenerationRef.current = false;
    }

    // Model loading events
    useEffect(() => {
        let unReady: null | (() => void) = null;
        let unErr: null | (() => void) = null;
        let unLoading: null | (() => void) = null;
        let unNoModel: null | (() => void) = null;

        (async () => {
            unLoading = await listen("model:loading", () => {
                console.log("[event] model:loading");
                setNoModelInstalled(false);
            });

            unReady = await listen("model:ready", () => {
                console.log("[event] model:ready");
                setModelReady(true);
                setNoModelInstalled(false);
            });

            unErr = await listen<string>("model:error", (e) => {
                console.log("[event] model:error", e.payload);
                setModelError(e.payload);
            });

            unNoModel = await listen("model:no_model", () => {
                console.log("[event] model:no_model");
                setNoModelInstalled(true);
                setModelReady(false);
            });
        })();

        return () => {
            unLoading?.();
            unReady?.();
            unErr?.();
            unNoModel?.();
        };
    }, []);

    // Model catalog events
    useEffect(() => {
        let unProgress: null | (() => void) = null;
        let unComplete: null | (() => void) = null;
        let unSwitching: null | (() => void) = null;
        let unModelsChanged: null | (() => void) = null;

        (async () => {
            unProgress = await listen<DownloadProgressPayload>("download:progress", (e) => {
                const { model_id, percent, speed_bps } = e.payload;
                setDownloadProgress((prev) => ({
                    ...prev,
                    [model_id]: { percent, speed: speed_bps },
                }));
            });

            unComplete = await listen<string>("download:complete", (e) => {
                const modelId = e.payload;
                console.log("[event] download:complete", modelId);
                setDownloadProgress((prev) => {
                    const next = { ...prev };
                    delete next[modelId];
                    return next;
                });
                refreshModels();
            });

            unSwitching = await listen<ModelSwitchPayload>("model:switching", (e) => {
                const { model_id, status, error } = e.payload;
                console.log("[event] model:switching", status, model_id, error);

                if (status === "ready") {
                    setModelSwitching(false);
                    setModelReady(true);
                    setNoModelInstalled(false);
                    setCurrentModelId(model_id);
                    refreshModels();
                } else if (status === "error") {
                    setModelSwitching(false);
                    console.error("Model switch error:", error);
                }
            });

            // Listen for file system changes in models directory
            unModelsChanged = await listen("models:changed", () => {
                console.log("[event] models:changed - refreshing model list");
                refreshModels();
            });
        })();

        return () => {
            unProgress?.();
            unComplete?.();
            unSwitching?.();
            unModelsChanged?.();
        };
    }, []);

    // Load models on mount and check if any are installed
    useEffect(() => {
        async function checkModels() {
            try {
                const modelList = await invoke<ModelInfo[]>("list_models");
                setModels(modelList);

                // Find current model
                const current = modelList.find((m) => m.is_current);
                if (current) {
                    setCurrentModelId(current.id);
                    setNoModelInstalled(false);
                } else {
                    // Check if any models are downloaded
                    const hasDownloaded = modelList.some((m) => m.download_status === "downloaded");
                    if (!hasDownloaded) {
                        setNoModelInstalled(true);
                    }
                }
            } catch (e) {
                console.log("[list_models] error", e);
            } finally {
                setInitialCheckDone(true);
            }
        }
        checkModels();
    }, []);

    // Poll model_status so we don't miss ready event (but stop if no model installed)
    useEffect(() => {
        let cancelled = false;

        async function poll() {
            if (noModelInstalled) {
                console.log("[poll] no model installed, stopping poll");
                return;
            }

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
    }, [noModelInstalled]);

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
                    { id: assistantId, role: "assistant", content: "", thinking: "", images: [], files: [], isStreaming: true },
                ]);

                setSelectedThinkingId(assistantId);
            });

            unlistenDeltaRef.current = await listen<ChatDeltaPayload>("chat:delta", (event) => {
                if (!mounted) return;
                if (!event.payload) return;
                if (event.payload.chat_id !== activeChatIdRef.current) return;

                const content_delta = event.payload.delta ?? "";
                const reasoning_delta = event.payload.reasoning_delta ?? "";
                const assistantId = currentAssistantIdRef.current;
                if (!assistantId) return;

                setMessages((prev) =>
                    prev.map((m) => {
                        if (m.id !== assistantId) return m;

                        return {
                            ...m,
                            content: m.content + content_delta,
                            thinking: m.thinking + reasoning_delta,
                        };
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
                        prev.map((m) =>
                            m.id === assistantId
                                ? { ...m, isStreaming: false, durationMs: event.payload.duration_ms }
                                : m
                        )
                    );
                }

                // Generate title for new chats (fire-and-forget, won't block)
                if (needsTitleGenerationRef.current) {
                    needsTitleGenerationRef.current = false;
                    const chatIdForTitle = event.payload.chat_id;
                    console.log("[generate_chat_title] requesting title for:", chatIdForTitle);
                    invoke("generate_chat_title", { args: { chatId: chatIdForTitle } })
                        .then(() => console.log("[generate_chat_title] completed for:", chatIdForTitle))
                        .catch((err) => console.error("[generate_chat_title] error:", err));
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

    // Global paste handler
    useEffect(() => {
        const handlePaste = (event: ClipboardEvent) => {
            const items = event.clipboardData?.items;
            if (!items) return;

            for (const item of items) {
                if (item.type.startsWith("image")) {
                    const file = item.getAsFile();
                    if (!file) continue;

                    event.preventDefault();

                    fileToBase64(file).then((base64) => {
                        setPendingImages((prev) => [
                            ...prev,
                            {
                                id: uid(),
                                base64,
                                previewUrl: `data:${file.type};base64,${base64}`,
                            },
                        ]);
                    });
                }
            }
        };

        window.addEventListener("paste", handlePaste);

        return () => {
            window.removeEventListener("paste", handlePaste);
        };
    }, []);



    const canSend = useMemo(
        () => (input.trim().length > 0 || pendingImages.length > 0 || pendingFiles.length > 0) && !isGenerating && modelReady && !modelSwitching,
        [input, pendingImages, pendingFiles, isGenerating, modelReady, modelSwitching]
    );

    // File type detection helpers
    const CODE_EXTENSIONS: Record<string, string> = {
        js: "javascript", jsx: "javascript", ts: "typescript", tsx: "typescript",
        py: "python", rb: "ruby", java: "java", c: "c", cpp: "cpp", h: "c",
        hpp: "cpp", cs: "csharp", go: "go", rs: "rust", swift: "swift",
        kt: "kotlin", scala: "scala", php: "php", sh: "bash", bash: "bash",
        zsh: "bash", fish: "fish", ps1: "powershell", sql: "sql", r: "r",
        lua: "lua", perl: "perl", hs: "haskell", ml: "ocaml", clj: "clojure",
        ex: "elixir", exs: "elixir", erl: "erlang", dart: "dart", vue: "vue",
        svelte: "svelte", css: "css", scss: "scss", sass: "sass", less: "less",
        html: "html", htm: "html", xml: "xml", json: "json", yaml: "yaml",
        yml: "yaml", toml: "toml", ini: "ini", cfg: "ini", conf: "ini",
        md: "markdown", mdx: "markdown", dockerfile: "dockerfile", makefile: "makefile",
    };

    const TEXT_EXTENSIONS = ["txt", "log", "csv", "tsv", "env", "gitignore", "editorconfig"];

    function getFileCategory(filename: string, mimeType: string): { type: "image" | "text" | "code" | "document" | "unsupported"; language?: string; docType?: string } {
        const ext = filename.split(".").pop()?.toLowerCase() || "";

        if (mimeType.startsWith("image/")) {
            return { type: "image" };
        }

        if (CODE_EXTENSIONS[ext]) {
            return { type: "code", language: CODE_EXTENSIONS[ext] };
        }

        if (TEXT_EXTENSIONS.includes(ext)) {
            return { type: "text" };
        }

        // Supported document types
        if (mimeType === "application/pdf" || ext === "pdf") {
            return { type: "document", docType: "pdf" };
        }

        if (mimeType === "application/vnd.openxmlformats-officedocument.wordprocessingml.document" || ext === "docx") {
            return { type: "document", docType: "docx" };
        }

        if (mimeType === "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" || ext === "xlsx" ||
            mimeType === "application/vnd.ms-excel" || ext === "xls") {
            return { type: "document", docType: "excel" };
        }

        // Unsupported binary formats
        if (ext === "doc") {
            return { type: "unsupported" }; // Old .doc format not supported
        }

        if (mimeType.startsWith("text/") || mimeType === "application/json" || mimeType === "application/xml") {
            return { type: "text" };
        }

        // Default to text for unknown types (may fail for binary files)
        return { type: "text" };
    }

    // Document parsing functions
    async function parsePdf(file: File): Promise<string> {
        const arrayBuffer = await file.arrayBuffer();
        const pdf = await pdfjsLib.getDocument({ data: arrayBuffer }).promise;
        const textParts: string[] = [];

        for (let i = 1; i <= pdf.numPages; i++) {
            const page = await pdf.getPage(i);
            const textContent = await page.getTextContent();
            const pageText = textContent.items
                .map((item: any) => item.str)
                .join(" ");
            textParts.push(`[Page ${i}]\n${pageText}`);
        }

        return textParts.join("\n\n");
    }

    async function parseDocx(file: File): Promise<string> {
        const arrayBuffer = await file.arrayBuffer();
        const result = await mammoth.extractRawText({ arrayBuffer });
        return result.value;
    }

    async function parseExcel(file: File): Promise<string> {
        const arrayBuffer = await file.arrayBuffer();
        const workbook = XLSX.read(arrayBuffer, { type: "array" });
        const textParts: string[] = [];

        for (const sheetName of workbook.SheetNames) {
            const sheet = workbook.Sheets[sheetName];
            const csv = XLSX.utils.sheet_to_csv(sheet);
            textParts.push(`[Sheet: ${sheetName}]\n${csv}`);
        }

        return textParts.join("\n\n");
    }

    async function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
        const files = e.target.files;
        if (!files) return;

        const unsupportedFiles: string[] = [];

        for (const file of Array.from(files)) {
            const category = getFileCategory(file.name, file.type);

            if (category.type === "unsupported") {
                unsupportedFiles.push(file.name);
                continue;
            }

            if (category.type === "image") {
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
            } else if (category.type === "document") {
                // Parse document files (PDF, DOCX, Excel)
                if (file.size > 50 * 1024 * 1024) {
                    console.warn("Document too large, skipping:", file.name);
                    continue;
                }

                try {
                    let content: string;

                    if (category.docType === "pdf") {
                        content = await parsePdf(file);
                    } else if (category.docType === "docx") {
                        content = await parseDocx(file);
                    } else if (category.docType === "excel") {
                        content = await parseExcel(file);
                    } else {
                        console.warn("Unknown document type:", file.name);
                        continue;
                    }

                    setPendingFiles((prev) => [
                        ...prev,
                        {
                            id: uid(),
                            name: file.name,
                            type: "document",
                            content,
                        },
                    ]);
                } catch (err) {
                    console.error("Failed to parse document:", file.name, err);
                    alert(`Failed to parse ${file.name}. The file may be corrupted or password-protected.`);
                }
            } else {
                // Text or code files
                if (file.size > 5 * 1024 * 1024) {
                    console.warn("File too large, skipping:", file.name);
                    continue;
                }

                try {
                    const content = await fileToText(file);
                    setPendingFiles((prev) => [
                        ...prev,
                        {
                            id: uid(),
                            name: file.name,
                            type: category.type as "text" | "code",
                            content,
                            language: category.language,
                        },
                    ]);
                } catch (err) {
                    console.error("Failed to read file:", file.name, err);
                }
            }
        }

        // Show warning for unsupported files
        if (unsupportedFiles.length > 0) {
            alert(`The following files are not supported:\n${unsupportedFiles.join("\n")}\n\nSupported: images, PDFs, Word docs (.docx), Excel files, text files, and code files.\n\nNote: Old .doc format is not supported, please convert to .docx.`);
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

    function fileToText(file: File): Promise<string> {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
                resolve(reader.result as string);
            };
            reader.onerror = reject;
            reader.readAsText(file);
        });
    }

    function removePendingImage(id: string) {
        setPendingImages((prev) => prev.filter((img) => img.id !== id));
    }

    function removePendingFile(id: string) {
        setPendingFiles((prev) => prev.filter((f) => f.id !== id));
    }

    async function handleSend() {
        const text = input.trim();
        if ((!text && pendingImages.length === 0 && pendingFiles.length === 0) || isGenerating || !modelReady) return;

        const userImages = [...pendingImages];
        const userFiles = [...pendingFiles];

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
        setPendingImages([]);
        setPendingFiles([]);

        try {
            setIsGenerating(true);

            let chat_id = chatId;

            if (chat_id === DRAFT_CHAT_ID) {
                chat_id = await invoke<string>("new_chat");
                setChatId(chat_id);
                needsTitleGenerationRef.current = true;
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
                    files: [],
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

    // Show loading screen only when we have a model and it's loading
    // Don't show loading if initial check isn't done yet or if no model is installed
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

    // Show a brief loading state while checking for models
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
                            <div className="historyItemContent">
                                <div className="historyTitle">{chat.title}</div>
                                <div className="historyPreview">{chat.preview}</div>
                                <div className="historyTime">
                                    {formatTimestamp(chat.updated_at)}
                                </div>
                            </div>
                            <button
                                className="deleteBtn"
                                onClick={(e) => handleDeleteChat(chat.id, e)}
                                title="Delete chat"
                            >
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" />
                                </svg>
                            </button>
                        </div>
                    ))}
                </div>

                <div className="sidebarFooter">
                    {modelCatalogOpen && (
                        <div className="modelCatalog">
                            <div className="modelCatalogHeader">
                                <span>Models</span>
                                <button
                                    className="closeCatalogBtn"
                                    onClick={() => setModelCatalogOpen(false)}
                                >
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                        <path d="M18 6L6 18M6 6l12 12" />
                                    </svg>
                                </button>
                            </div>
                            <div className="modelList niceScroll">
                                {models.length === 0 ? (
                                    <div className="modelListEmpty">No models in catalog</div>
                                ) : (
                                    models.map((model) => {
                                        const progress = downloadProgress[model.id];
                                        const isDownloading = model.download_status === "downloading" || progress;
                                        const isDownloaded = model.download_status === "downloaded";
                                        const isCurrent = model.is_current;

                                        return (
                                            <div
                                                key={model.id}
                                                className={`modelItem ${isCurrent ? "current" : ""} ${isDownloaded && !isCurrent ? "clickable" : ""}`}
                                                onClick={() => {
                                                    if (isDownloaded && !isCurrent && !modelSwitching) {
                                                        handleSwitchModel(model.id);
                                                        setModelCatalogOpen(false);
                                                    }
                                                }}
                                            >
                                                <div className="modelItemHeader">
                                                    <div className="modelItemTitle">
                                                        <span className="modelName">{model.name}</span>
                                                        {model.size_label && (
                                                            <span className="modelSize">{model.size_label}</span>
                                                        )}
                                                    </div>
                                                    <div className="modelBadges">
                                                        {model.capabilities.vision && (
                                                            <span className="capabilityBadge vision" title="Vision capable">
                                                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                                                                    <circle cx="12" cy="12" r="3" />
                                                                </svg>
                                                            </span>
                                                        )}
                                                        {model.capabilities.thinking && (
                                                            <span className="capabilityBadge thinking" title="Extended thinking">
                                                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                                    <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                                                                </svg>
                                                            </span>
                                                        )}
                                                        {isCurrent && (
                                                            <span className="currentBadge">Active</span>
                                                        )}
                                                    </div>
                                                </div>
                                                <div className="modelDescription">{model.description}</div>

                                                {isDownloading && (
                                                    <div className="modelDownloadProgress">
                                                        <div className="progressBar">
                                                            <div
                                                                className="progressFill"
                                                                style={{ width: `${progress?.percent ?? model.download_percent ?? 0}%` }}
                                                            />
                                                        </div>
                                                        <div className="progressInfo">
                                                            <span>{(progress?.percent ?? model.download_percent ?? 0).toFixed(1)}%</span>
                                                            {progress && <span>{formatSpeed(progress.speed)}</span>}
                                                            <button
                                                                className="cancelDownloadBtn"
                                                                onClick={(e) => {
                                                                    e.stopPropagation();
                                                                    handleCancelDownload(model.id);
                                                                }}
                                                            >
                                                                Cancel
                                                            </button>
                                                        </div>
                                                    </div>
                                                )}

                                                {!isDownloaded && !isDownloading && (
                                                    <button
                                                        className="downloadBtn"
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            handleDownloadModel(model.id);
                                                        }}
                                                    >
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3" />
                                                        </svg>
                                                        Download
                                                    </button>
                                                )}

                                                {isDownloaded && !isCurrent && (
                                                    <button
                                                        className="deleteModelBtn"
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            if (confirm(`Delete ${model.name}?`)) {
                                                                handleDeleteModel(model.id);
                                                            }
                                                        }}
                                                        title="Delete model"
                                                    >
                                                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                            <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" />
                                                        </svg>
                                                    </button>
                                                )}
                                            </div>
                                        );
                                    })
                                )}
                            </div>
                        </div>
                    )}

                    <div
                        className={`userSection ${modelCatalogOpen ? "active" : ""} ${noModelInstalled ? "warning" : ""}`}
                        onClick={() => setModelCatalogOpen(!modelCatalogOpen)}
                    >
                        <div className={`userAvatar ${noModelInstalled ? "warning" : ""}`}>E</div>
                        <div className="userInfo">
                            <div className="userName">Eigen</div>
                            <div className={`currentModel ${noModelInstalled ? "warning" : ""}`}>
                                {modelSwitching ? "Switching..." : currentModelName}
                            </div>
                        </div>
                        <svg
                            className={`chevron ${modelCatalogOpen ? "open" : ""}`}
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                        >
                            <path d="M18 15l-6-6-6 6" />
                        </svg>
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
                    {noModelInstalled && (
                        <div className="noModelWarning">
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
                                <line x1="12" y1="9" x2="12" y2="13" />
                                <line x1="12" y1="17" x2="12.01" y2="17" />
                            </svg>
                            <div className="noModelWarningContent">
                                <div className="noModelWarningTitle">No model active</div>
                                <div className="noModelWarningText">
                                    Click the <strong>Eigen</strong> button below to download a model and start chatting.
                                </div>
                            </div>
                        </div>
                    )}
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

                                    {/* Display files if present */}
                                    {m.files && m.files.length > 0 && (
                                        <div className="messageFiles">
                                            {m.files.map((file) => (
                                                <div key={file.id} className={`messageFileChip ${file.type === "document" ? "document" : ""}`}>
                                                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                        {file.type === "code" ? (
                                                            <><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></>
                                                        ) : file.type === "document" ? (
                                                            <><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><line x1="16" y1="13" x2="8" y2="13" /><line x1="16" y1="17" x2="8" y2="17" /></>
                                                        ) : (
                                                            <><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></>
                                                        )}
                                                    </svg>
                                                    {file.name}
                                                </div>
                                            ))}
                                        </div>
                                    )}

                                    <div
                                        className={`bubble ${isUser ? "userBubble" : "assistantBubble"} ${ !isUser && m.id === selectedThinkingId ? "selected" : ""}`}
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

                                    {!isUser && !m.isStreaming && (
                                        <div className="msgMeta">
                                            <button className="thinkBtn" onClick={() => setSelectedThinkingId(m.id)}>
                                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                    <circle cx="12" cy="12" r="3" />
                                                    <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
                                                </svg>
                                                View thinking
                                            </button>

                                            {m.durationMs && (
                                                <div className="duration">
                                                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                        <circle cx="12" cy="12" r="10" />
                                                        <polyline points="12 6 12 12 16 14" />
                                                    </svg>
                                                    {(m.durationMs / 1000).toFixed(1)}s
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {isUser && <div className="msgAvatar user">U</div>}
                            </div>
                        );
                    })}
                    <div ref={endRef} />
                </div>

                <div className="inputRow">
                    {/* Pending attachments preview */}
                    {(pendingImages.length > 0 || pendingFiles.length > 0) && (
                        <div className="pendingAttachments">
                            {pendingImages.map((img) => (
                                <div key={img.id} className="pendingImageThumb">
                                    <img src={img.previewUrl} alt="pending" />
                                    <button onClick={() => removePendingImage(img.id)} title="Remove image">
                                        &times;
                                    </button>
                                </div>
                            ))}
                            {pendingFiles.map((file) => (
                                <div key={file.id} className={`pendingFileChip ${file.type === "document" ? "document" : ""}`}>
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                        {file.type === "code" ? (
                                            <><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></>
                                        ) : file.type === "document" ? (
                                            <><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><line x1="16" y1="13" x2="8" y2="13" /><line x1="16" y1="17" x2="8" y2="17" /><line x1="10" y1="9" x2="8" y2="9" /></>
                                        ) : (
                                            <><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></>
                                        )}
                                    </svg>
                                    <span className="pendingFileName">{file.name}</span>
                                    <button onClick={() => removePendingFile(file.id)} title="Remove file">
                                        &times;
                                    </button>
                                </div>
                            ))}
                        </div>
                    )}

                    <div className="inputContainer">
                        <button
                            className="fileUploadBtn"
                            onClick={() => fileInputRef.current?.click()}
                            disabled={isGenerating}
                            title="Upload file (images, code, documents)"
                        >
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48" />
                            </svg>
                        </button>
                        <input
                            type="file"
                            ref={fileInputRef}
                            accept="image/*,.pdf,.docx,.xlsx,.xls,.txt,.md,.json,.xml,.csv,.tsv,.log,.env,.py,.js,.ts,.tsx,.jsx,.c,.cpp,.h,.hpp,.java,.rb,.go,.rs,.swift,.kt,.scala,.php,.sh,.bash,.zsh,.sql,.r,.lua,.pl,.hs,.ml,.clj,.ex,.exs,.erl,.dart,.vue,.svelte,.html,.htm,.css,.scss,.sass,.less,.yaml,.yml,.toml,.ini,.cfg,.conf,.gitignore,.editorconfig,Dockerfile,Makefile"
                            multiple
                            onChange={handleFileSelect}
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