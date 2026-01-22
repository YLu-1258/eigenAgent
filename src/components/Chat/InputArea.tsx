// src/components/Chat/InputArea.tsx

import { ImageAttachment, FileAttachment } from "../../types/chat";

interface InputAreaProps {
    input: string;
    pendingImages: ImageAttachment[];
    pendingFiles: FileAttachment[];
    isGenerating: boolean;
    canSend: boolean;
    fileInputRef: React.RefObject<HTMLInputElement | null>;
    onInputChange: (value: string) => void;
    onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
    onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onRemovePendingImage: (id: string) => void;
    onRemovePendingFile: (id: string) => void;
    onSend: () => void;
    onStop: () => void;
}

export function InputArea({
    input,
    pendingImages,
    pendingFiles,
    isGenerating,
    canSend,
    fileInputRef,
    onInputChange,
    onKeyDown,
    onFileSelect,
    onRemovePendingImage,
    onRemovePendingFile,
    onSend,
    onStop,
}: InputAreaProps) {
    return (
        <div className="inputRow">
            {/* Pending attachments preview */}
            {(pendingImages.length > 0 || pendingFiles.length > 0) && (
                <div className="pendingAttachments">
                    {pendingImages.map((img) => (
                        <div key={img.id} className="pendingImageThumb">
                            <img src={img.previewUrl} alt="pending" />
                            <button onClick={() => onRemovePendingImage(img.id)} title="Remove image">
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
                            <button onClick={() => onRemovePendingFile(file.id)} title="Remove file">
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
                    onChange={onFileSelect}
                    style={{ display: "none" }}
                />
                <input
                    value={input}
                    onChange={(e) => onInputChange(e.target.value)}
                    onKeyDown={onKeyDown}
                    placeholder="Ask anything..."
                    className="input"
                    disabled={isGenerating}
                />
                {isGenerating ? (
                    <button className="stopBtn" onClick={onStop} title="Stop generating">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                            <rect x="6" y="6" width="12" height="12" rx="2" />
                        </svg>
                    </button>
                ) : (
                    <button className={`sendBtn ${canSend ? "active" : ""}`} onClick={onSend} disabled={!canSend}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z" />
                        </svg>
                    </button>
                )}
            </div>
        </div>
    );
}
