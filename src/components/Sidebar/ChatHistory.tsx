// src/components/Sidebar/ChatHistory.tsx

import { ChatHistoryItem } from "../../types/chat";
import { formatTimestamp } from "../../utils/format";

interface ChatHistoryProps {
    chatHistory: ChatHistoryItem[];
    currentChatId: string;
    onLoadChat: (chatId: string) => void;
    onDeleteChat: (chatId: string, e: React.MouseEvent) => void;
}

export function ChatHistory({ chatHistory, currentChatId, onLoadChat, onDeleteChat }: ChatHistoryProps) {
    return (
        <div className="historySection niceScroll">
            <div className="historyLabel">Recent</div>

            {chatHistory.map((chat) => (
                <div
                    key={chat.id}
                    className={`historyItem ${currentChatId === chat.id ? "active" : ""}`}
                    onClick={() => {
                        if (currentChatId !== chat.id) onLoadChat(chat.id);
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
                        onClick={(e) => onDeleteChat(chat.id, e)}
                        title="Delete chat"
                    >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" />
                        </svg>
                    </button>
                </div>
            ))}
        </div>
    );
}
