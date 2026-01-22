// src/components/Chat/MessageList.tsx

import { useRef } from "react";
import { ChatMessage } from "../../types/chat";
import { Message } from "./Message";

interface MessageListProps {
    messages: ChatMessage[];
    selectedThinkingId: string | null;
    noModelInstalled: boolean;
    onSelectThinking: (messageId: string) => void;
}

export function MessageList({ messages, selectedThinkingId, noModelInstalled, onSelectThinking }: MessageListProps) {
    const endRef = useRef<HTMLDivElement | null>(null);

    return (
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
            {messages.map((m) => (
                <Message
                    key={m.id}
                    message={m}
                    isSelected={m.id === selectedThinkingId}
                    onSelectThinking={() => onSelectThinking(m.id)}
                />
            ))}
            <div ref={endRef} />
        </div>
    );
}
