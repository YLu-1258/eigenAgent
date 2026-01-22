// src/components/Chat/Message.tsx

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { ChatMessage } from "../../types/chat";

interface MessageProps {
    message: ChatMessage;
    isSelected: boolean;
    onSelectThinking: () => void;
}

export function Message({ message, isSelected, onSelectThinking }: MessageProps) {
    const isUser = message.role === "user";
    const showPlaceholder = !isUser && message.isStreaming && message.content.trim().length === 0;

    return (
        <div className={`msgRow ${isUser ? "right" : "left"}`}>
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
                {message.images && message.images.length > 0 && (
                    <div className="messageImages">
                        {message.images.map((img) => (
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
                {message.files && message.files.length > 0 && (
                    <div className="messageFiles">
                        {message.files.map((file) => (
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
                    className={`bubble ${isUser ? "userBubble" : "assistantBubble"} ${!isUser && isSelected ? "selected" : ""}`}
                    title={!isUser ? "Click to view thinking" : undefined}
                    onClick={() => {
                        if (!isUser) onSelectThinking();
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
                                {message.content}
                            </ReactMarkdown>
                        </div>
                    ) : (
                        <span className="userText">{message.content}</span>
                    )}
                </div>

                {!isUser && !message.isStreaming && (
                    <div className="msgMeta">
                        <button className="thinkBtn" onClick={onSelectThinking}>
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <circle cx="12" cy="12" r="3" />
                                <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
                            </svg>
                            View thinking
                        </button>

                        {message.durationMs && (
                            <div className="duration">
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <circle cx="12" cy="12" r="10" />
                                    <polyline points="12 6 12 12 16 14" />
                                </svg>
                                {(message.durationMs / 1000).toFixed(1)}s
                            </div>
                        )}
                    </div>
                )}
            </div>

            {isUser && <div className="msgAvatar user">U</div>}
        </div>
    );
}
