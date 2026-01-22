// src/components/Sidebar/index.tsx

import { useState } from "react";
import { ChatHistoryItem } from "../../types/chat";
import { ModelInfo } from "../../types/model";
import { ChatHistory } from "./ChatHistory";
import { ModelCatalog } from "./ModelCatalog";

interface SidebarProps {
    isOpen: boolean;
    chatHistory: ChatHistoryItem[];
    currentChatId: string;
    models: ModelInfo[];
    currentModelName: string;
    noModelInstalled: boolean;
    modelSwitching: boolean;
    downloadProgress: Record<string, { percent: number; speed: number }>;
    onToggle: () => void;
    onNewChat: () => void;
    onLoadChat: (chatId: string) => void;
    onDeleteChat: (chatId: string, e: React.MouseEvent) => void;
    onSwitchModel: (modelId: string) => void;
    onDownloadModel: (modelId: string) => void;
    onCancelDownload: (modelId: string) => void;
    onDeleteModel: (modelId: string) => void;
    onOpenSettings: () => void;
}

export function Sidebar({
    isOpen,
    chatHistory,
    currentChatId,
    models,
    currentModelName,
    noModelInstalled,
    modelSwitching,
    downloadProgress,
    onToggle,
    onNewChat,
    onLoadChat,
    onDeleteChat,
    onSwitchModel,
    onDownloadModel,
    onCancelDownload,
    onDeleteModel,
    onOpenSettings,
}: SidebarProps) {
    const [modelCatalogOpen, setModelCatalogOpen] = useState(false);

    return (
        <div className={`sidebarWrapper ${isOpen ? "open" : "closed"}`}>
            <div className={`sidebar ${isOpen ? "open" : "closed"}`}>
                <div className="sidebarHeader">
                    <button className="newChatBtn" onClick={onNewChat}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M12 5v14M5 12h14" />
                        </svg>
                        New chat
                    </button>
                </div>

                <ChatHistory
                    chatHistory={chatHistory}
                    currentChatId={currentChatId}
                    onLoadChat={onLoadChat}
                    onDeleteChat={onDeleteChat}
                />

                <div className="sidebarFooter">
                    {modelCatalogOpen && (
                        <ModelCatalog
                            models={models}
                            downloadProgress={downloadProgress}
                            modelSwitching={modelSwitching}
                            onClose={() => setModelCatalogOpen(false)}
                            onSwitchModel={onSwitchModel}
                            onDownloadModel={onDownloadModel}
                            onCancelDownload={onCancelDownload}
                            onDeleteModel={onDeleteModel}
                        />
                    )}

                    <div className="sidebarFooterRow">
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
                        <button
                            className="settingsBtn"
                            onClick={onOpenSettings}
                            title="Settings (Cmd+,)"
                        >
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <circle cx="12" cy="12" r="3" />
                                <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>

            {/* TOGGLE SIDEBAR BUTTON */}
            <button
                className="sidebarToggle"
                onClick={onToggle}
                title={isOpen ? "Close sidebar" : "Open sidebar"}
            >
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    {isOpen ? <path d="M15 18l-6-6 6-6" /> : <path d="M9 18l6-6-6-6" />}
                </svg>
            </button>
        </div>
    );
}
