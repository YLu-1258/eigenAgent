// src/components/ThinkingPanel.tsx

import { ChatMessage } from "../types/chat";

interface ThinkingPanelProps {
    selectedMessage: ChatMessage | null;
}

export function ThinkingPanel({ selectedMessage }: ThinkingPanelProps) {
    return (
        <div className="thinkingCol">
            <div className="thinkingHeader">
                <div className="thinkingTitle">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                    </svg>
                    Thinking Process
                </div>
                <div className="thinkingStatus">
                    {selectedMessage?.isStreaming && <span className="streamingBadge">Live</span>}
                </div>
            </div>

            {!selectedMessage ? (
                <div className="thinkingEmpty">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                        <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                    </svg>
                    <div>Click on any assistant message to see its thinking process</div>
                </div>
            ) : (
                <div className="thinkingBox niceScroll">
                    {selectedMessage.thinking.trim().length > 0
                        ? selectedMessage.thinking
                        : "No thinking captured for this message."}
                </div>
            )}
        </div>
    );
}
