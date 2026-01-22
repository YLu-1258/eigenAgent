// src/components/Sidebar/ModelCatalog.tsx

import { ModelInfo } from "../../types/model";
import { formatSpeed } from "../../utils/format";

interface ModelCatalogProps {
    models: ModelInfo[];
    downloadProgress: Record<string, { percent: number; speed: number }>;
    modelSwitching: boolean;
    onClose: () => void;
    onSwitchModel: (modelId: string) => void;
    onDownloadModel: (modelId: string) => void;
    onCancelDownload: (modelId: string) => void;
    onDeleteModel: (modelId: string) => void;
}

export function ModelCatalog({
    models,
    downloadProgress,
    modelSwitching,
    onClose,
    onSwitchModel,
    onDownloadModel,
    onCancelDownload,
    onDeleteModel,
}: ModelCatalogProps) {
    return (
        <div className="modelCatalog">
            <div className="modelCatalogHeader">
                <span>Models</span>
                <button className="closeCatalogBtn" onClick={onClose}>
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
                                        onSwitchModel(model.id);
                                        onClose();
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
                                                    onCancelDownload(model.id);
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
                                            onDownloadModel(model.id);
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
                                                onDeleteModel(model.id);
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
    );
}
