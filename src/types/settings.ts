// src/types/settings.ts

export type Theme = "dark" | "light" | "system";
export type FontSize = "small" | "medium" | "large";

export interface AppearanceSettings {
    theme: Theme;
    accentColor: string;
    fontSize: FontSize;
}

export interface DefaultSettings {
    modelId: string | null;
    systemPrompt: string;
}

export interface BehaviorSettings {
    sendOnEnter: boolean;
    streamingEnabled: boolean;
    contextLength: number;
}

export interface AppSettings {
    version: number;
    appearance: AppearanceSettings;
    defaults: DefaultSettings;
    behavior: BehaviorSettings;
}

export const DEFAULT_SYSTEM_PROMPT = `You are Eigen, a helpful AI assistant.

Rules:
- Use Markdown for formatting.
- Use LaTeX ($...$ / $$...$$) for math.
- If you don't know, say "I don't know".`;

export const ACCENT_COLOR_PRESETS = [
    { name: "Blue", hex: "#3b82f6" },
    { name: "Purple", hex: "#8b5cf6" },
    { name: "Green", hex: "#10b981" },
    { name: "Orange", hex: "#f97316" },
    { name: "Pink", hex: "#ec4899" },
    { name: "Cyan", hex: "#06b6d4" },
] as const;

export const getDefaultSettings = (): AppSettings => ({
    version: 1,
    appearance: {
        theme: "dark",
        accentColor: "#3b82f6",
        fontSize: "medium",
    },
    defaults: {
        modelId: null,
        systemPrompt: DEFAULT_SYSTEM_PROMPT,
    },
    behavior: {
        sendOnEnter: true,
        streamingEnabled: true,
        contextLength: 8192,
    },
});
