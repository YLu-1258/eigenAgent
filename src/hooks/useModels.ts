// src/hooks/useModels.ts

import { useState, useEffect, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ModelInfo, DownloadProgressPayload, ModelSwitchPayload } from "../types/model";

interface UseModelsReturn {
    // State
    models: ModelInfo[];
    currentModelId: string | null;
    currentModelName: string;
    modelReady: boolean;
    modelError: string | null;
    modelSwitching: boolean;
    noModelInstalled: boolean;
    initialCheckDone: boolean;
    downloadProgress: Record<string, { percent: number; speed: number }>;

    // Actions
    refreshModels: () => Promise<void>;
    switchModel: (modelId: string) => Promise<void>;
    downloadModel: (modelId: string) => Promise<void>;
    cancelDownload: (modelId: string) => Promise<void>;
    deleteModel: (modelId: string) => Promise<void>;
}

export function useModels(isGenerating: boolean): UseModelsReturn {
    const [models, setModels] = useState<ModelInfo[]>([]);
    const [currentModelId, setCurrentModelId] = useState<string | null>(null);
    const [modelReady, setModelReady] = useState(false);
    const [modelError, setModelError] = useState<string | null>(null);
    const [modelSwitching, setModelSwitching] = useState(false);
    const [noModelInstalled, setNoModelInstalled] = useState(false);
    const [initialCheckDone, setInitialCheckDone] = useState(false);
    const [downloadProgress, setDownloadProgress] = useState<Record<string, { percent: number; speed: number }>>({});

    // Computed value for current model name
    const currentModelName = useMemo(() => {
        if (noModelInstalled || !currentModelId) return "No model active!";
        const model = models.find((m) => m.id === currentModelId);
        return model?.name || currentModelId;
    }, [currentModelId, models, noModelInstalled]);

    const refreshModels = useCallback(async () => {
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
    }, []);

    const switchModel = useCallback(async (modelId: string) => {
        if (modelSwitching || isGenerating) return;

        try {
            setModelSwitching(true);
            setModelReady(false);
            await invoke("switch_model", { args: { modelId } });
        } catch (e) {
            console.error("[switch_model] error", e);
            setModelSwitching(false);
        }
    }, [modelSwitching, isGenerating]);

    const downloadModel = useCallback(async (modelId: string) => {
        try {
            await invoke("download_model", { args: { modelId } });
        } catch (e) {
            console.error("[download_model] error", e);
        }
    }, []);

    const cancelDownload = useCallback(async (modelId: string) => {
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
    }, []);

    const deleteModel = useCallback(async (modelId: string) => {
        try {
            await invoke("delete_model", { args: { modelId } });
            await refreshModels();
        } catch (e) {
            console.error("[delete_model] error", e);
        }
    }, [refreshModels]);

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
    }, [refreshModels]);

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

    return {
        models,
        currentModelId,
        currentModelName,
        modelReady,
        modelError,
        modelSwitching,
        noModelInstalled,
        initialCheckDone,
        downloadProgress,
        refreshModels,
        switchModel,
        downloadModel,
        cancelDownload,
        deleteModel,
    };
}
