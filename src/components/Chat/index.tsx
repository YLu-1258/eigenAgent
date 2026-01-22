// src/components/Chat/index.tsx

import { ChatMessage, ImageAttachment, FileAttachment } from "../../types/chat";
import { MessageList } from "./MessageList";
import { InputArea } from "./InputArea";

interface ChatProps {
    messages: ChatMessage[];
    selectedThinkingId: string | null;
    noModelInstalled: boolean;
    input: string;
    pendingImages: ImageAttachment[];
    pendingFiles: FileAttachment[];
    isGenerating: boolean;
    canSend: boolean;
    fileInputRef: React.RefObject<HTMLInputElement | null>;
    onSelectThinking: (messageId: string) => void;
    onInputChange: (value: string) => void;
    onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
    onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onRemovePendingImage: (id: string) => void;
    onRemovePendingFile: (id: string) => void;
    onSend: () => void;
    onStop: () => void;
}

export function Chat({
    messages,
    selectedThinkingId,
    noModelInstalled,
    input,
    pendingImages,
    pendingFiles,
    isGenerating,
    canSend,
    fileInputRef,
    onSelectThinking,
    onInputChange,
    onKeyDown,
    onFileSelect,
    onRemovePendingImage,
    onRemovePendingFile,
    onSend,
    onStop,
}: ChatProps) {
    return (
        <div className="chatCol">
            <div className="chatHeader">
                <div className="chatTitle">Eigen</div>
                <div className="statusIndicator">
                    <div className="statusDot"></div>
                    Online
                </div>
            </div>

            <MessageList
                messages={messages}
                selectedThinkingId={selectedThinkingId}
                noModelInstalled={noModelInstalled}
                onSelectThinking={onSelectThinking}
            />

            <InputArea
                input={input}
                pendingImages={pendingImages}
                pendingFiles={pendingFiles}
                isGenerating={isGenerating}
                canSend={canSend}
                fileInputRef={fileInputRef}
                onInputChange={onInputChange}
                onKeyDown={onKeyDown}
                onFileSelect={onFileSelect}
                onRemovePendingImage={onRemovePendingImage}
                onRemovePendingFile={onRemovePendingFile}
                onSend={onSend}
                onStop={onStop}
            />
        </div>
    );
}
