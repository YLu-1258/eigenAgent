// src/stores/settingsStore.ts

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings, getDefaultSettings, Theme, FontSize } from "../types/settings";

interface SettingsState {
    settings: AppSettings;
    isLoading: boolean;
    error: string | null;

    // Actions
    loadSettings: () => Promise<void>;
    saveSettings: (settings: AppSettings) => Promise<void>;
    resetSettings: () => Promise<void>;

    // Convenience setters
    setTheme: (theme: Theme) => Promise<void>;
    setAccentColor: (color: string) => Promise<void>;
    setFontSize: (size: FontSize) => Promise<void>;
    setSystemPrompt: (prompt: string) => Promise<void>;
    setDefaultModelId: (modelId: string | null) => Promise<void>;
    setSendOnEnter: (value: boolean) => Promise<void>;
    setStreamingEnabled: (value: boolean) => Promise<void>;
    setContextLength: (length: number) => Promise<void>;
}

// Apply theme to document
function applyTheme(settings: AppSettings): void {
    const { theme, accentColor, fontSize } = settings.appearance;

    // Determine effective theme
    let effectiveTheme: "dark" | "light" = "dark";
    if (theme === "system") {
        effectiveTheme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    } else {
        effectiveTheme = theme;
    }

    // Apply theme attribute
    document.documentElement.setAttribute("data-theme", effectiveTheme);
    document.documentElement.setAttribute("data-font-size", fontSize);

    // Apply accent color as CSS custom property
    document.documentElement.style.setProperty("--color-accent", accentColor);

    // Calculate hover/light/dark variants
    const rgb = hexToRgb(accentColor);
    if (rgb) {
        // Store RGB values for use in rgba()
        document.documentElement.style.setProperty("--accent-r", String(rgb.r));
        document.documentElement.style.setProperty("--accent-g", String(rgb.g));
        document.documentElement.style.setProperty("--accent-b", String(rgb.b));

        // Lighter version for hover
        const lighterHex = rgbToHex(
            Math.min(255, rgb.r + 40),
            Math.min(255, rgb.g + 40),
            Math.min(255, rgb.b + 40)
        );
        document.documentElement.style.setProperty("--color-accent-hover", lighterHex);

        // Even lighter for light variant
        const lightHex = rgbToHex(
            Math.min(255, rgb.r + 80),
            Math.min(255, rgb.g + 80),
            Math.min(255, rgb.b + 80)
        );
        document.documentElement.style.setProperty("--color-accent-light", lightHex);

        // Darker version
        const darkerHex = rgbToHex(
            Math.max(0, rgb.r - 30),
            Math.max(0, rgb.g - 30),
            Math.max(0, rgb.b - 30)
        );
        document.documentElement.style.setProperty("--color-accent-dark", darkerHex);

        // Even darker for gradients
        const darkerHex2 = rgbToHex(
            Math.max(0, rgb.r - 50),
            Math.max(0, rgb.g - 50),
            Math.max(0, rgb.b - 50)
        );
        document.documentElement.style.setProperty("--color-accent-darker", darkerHex2);

        // Pre-computed rgba values for common opacities
        document.documentElement.style.setProperty("--color-accent-5", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.05)`);
        document.documentElement.style.setProperty("--color-accent-10", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.1)`);
        document.documentElement.style.setProperty("--color-accent-15", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.15)`);
        document.documentElement.style.setProperty("--color-accent-20", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.2)`);
        document.documentElement.style.setProperty("--color-accent-25", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.25)`);
        document.documentElement.style.setProperty("--color-accent-30", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
        document.documentElement.style.setProperty("--color-accent-40", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.4)`);
        document.documentElement.style.setProperty("--color-accent-50", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`);
        document.documentElement.style.setProperty("--color-accent-60", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.6)`);

        // Border colors based on accent
        document.documentElement.style.setProperty(
            "--color-border",
            `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.1)`
        );
        document.documentElement.style.setProperty(
            "--color-border-accent",
            `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`
        );
        document.documentElement.style.setProperty(
            "--color-border-accent-hover",
            `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`
        );

        // Glow/shadow colors
        document.documentElement.style.setProperty(
            "--color-glow",
            `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.4)`
        );
        document.documentElement.style.setProperty(
            "--color-glow-strong",
            `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.6)`
        );
    }

    // Store theme in localStorage for pre-render access
    localStorage.setItem("eigenAgent-theme", effectiveTheme);
    localStorage.setItem("eigenAgent-fontSize", fontSize);
    localStorage.setItem("eigenAgent-accentColor", accentColor);
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
        ? {
              r: parseInt(result[1], 16),
              g: parseInt(result[2], 16),
              b: parseInt(result[3], 16),
          }
        : null;
}

function rgbToHex(r: number, g: number, b: number): string {
    return "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
    settings: getDefaultSettings(),
    isLoading: false,
    error: null,

    loadSettings: async () => {
        set({ isLoading: true, error: null });
        try {
            const settings = await invoke<AppSettings>("cmd_load_settings");
            set({ settings, isLoading: false });
            applyTheme(settings);
        } catch (e) {
            console.error("[settings] Failed to load:", e);
            set({ error: String(e), isLoading: false });
        }
    },

    saveSettings: async (settings: AppSettings) => {
        try {
            await invoke("cmd_save_settings", { newSettings: settings });
            set({ settings });
            applyTheme(settings);
        } catch (e) {
            console.error("[settings] Failed to save:", e);
            set({ error: String(e) });
        }
    },

    resetSettings: async () => {
        try {
            const settings = await invoke<AppSettings>("cmd_reset_settings");
            set({ settings });
            applyTheme(settings);
        } catch (e) {
            console.error("[settings] Failed to reset:", e);
            set({ error: String(e) });
        }
    },

    setTheme: async (theme: Theme) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            appearance: { ...settings.appearance, theme },
        };
        await saveSettings(newSettings);
    },

    setAccentColor: async (accentColor: string) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            appearance: { ...settings.appearance, accentColor },
        };
        await saveSettings(newSettings);
    },

    setFontSize: async (fontSize: FontSize) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            appearance: { ...settings.appearance, fontSize },
        };
        await saveSettings(newSettings);
    },

    setSystemPrompt: async (systemPrompt: string) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            defaults: { ...settings.defaults, systemPrompt },
        };
        await saveSettings(newSettings);
    },

    setDefaultModelId: async (modelId: string | null) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            defaults: { ...settings.defaults, modelId },
        };
        await saveSettings(newSettings);
    },

    setSendOnEnter: async (sendOnEnter: boolean) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            behavior: { ...settings.behavior, sendOnEnter },
        };
        await saveSettings(newSettings);
    },

    setStreamingEnabled: async (streamingEnabled: boolean) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            behavior: { ...settings.behavior, streamingEnabled },
        };
        await saveSettings(newSettings);
    },

    setContextLength: async (contextLength: number) => {
        const { settings, saveSettings } = get();
        const newSettings = {
            ...settings,
            behavior: { ...settings.behavior, contextLength },
        };
        await saveSettings(newSettings);
    },
}));

// Listen for system theme changes
if (typeof window !== "undefined") {
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
        const { settings } = useSettingsStore.getState();
        if (settings.appearance.theme === "system") {
            applyTheme(settings);
        }
    });
}
