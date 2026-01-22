// src/types/model.ts

export type ModelCapabilities = {
    vision: boolean;
    thinking: boolean;
};

export type ModelInfo = {
    id: string;
    name: string;
    description: string;
    size_label: string;
    capabilities: ModelCapabilities;
    download_status: string; // "not_downloaded" | "downloading" | "downloaded"
    download_percent: number | null;
    is_current: boolean;
};

export type DownloadProgressPayload = {
    model_id: string;
    downloaded_bytes: number;
    total_bytes: number;
    percent: number;
    speed_bps: number;
};

export type ModelSwitchPayload = {
    model_id: string;
    status: string; // "stopping" | "starting" | "ready" | "error"
    error?: string;
};
