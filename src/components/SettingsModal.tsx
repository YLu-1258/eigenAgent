// src/components/SettingsModal.tsx

import { useEffect, useState } from "react";
import { useSettingsStore } from "../stores/settingsStore";
import { Theme, FontSize, AppSettings, ACCENT_COLOR_PRESETS, DEFAULT_SYSTEM_PROMPT } from "../types/settings";

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    models: Array<{ id: string; name: string }>;
}

export function SettingsModal({ isOpen, onClose, models }: SettingsModalProps) {
    const { settings, saveSettings, resetSettings } = useSettingsStore();

    // Local state for all settings (not persisted until Save)
    const [localSettings, setLocalSettings] = useState<AppSettings>(settings);
    const [hasChanges, setHasChanges] = useState(false);

    // Sync local state when modal opens or settings change externally
    useEffect(() => {
        if (isOpen) {
            setLocalSettings(settings);
            setHasChanges(false);
        }
    }, [isOpen, settings]);

    // Apply theme preview in real-time (but don't persist)
    useEffect(() => {
        if (isOpen) {
            applyThemePreview(localSettings);
        }
    }, [isOpen, localSettings.appearance]);

    // Handle escape key
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape" && isOpen) {
                handleCancel();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [isOpen, hasChanges]);

    if (!isOpen) return null;

    // Update local settings and mark as changed

    function updateAppearance(updates: Partial<AppSettings["appearance"]>) {
        setLocalSettings(prev => ({
            ...prev,
            appearance: { ...prev.appearance, ...updates }
        }));
        setHasChanges(true);
    }

    function updateDefaults(updates: Partial<AppSettings["defaults"]>) {
        setLocalSettings(prev => ({
            ...prev,
            defaults: { ...prev.defaults, ...updates }
        }));
        setHasChanges(true);
    }

    function updateBehavior(updates: Partial<AppSettings["behavior"]>) {
        setLocalSettings(prev => ({
            ...prev,
            behavior: { ...prev.behavior, ...updates }
        }));
        setHasChanges(true);
    }

    async function handleSave() {
        await saveSettings(localSettings);
        setHasChanges(false);
        onClose();
    }

    function handleCancel() {
        // Revert theme preview to saved settings
        applyThemePreview(settings);
        setLocalSettings(settings);
        setHasChanges(false);
        onClose();
    }

    async function handleResetAll() {
        try {
            console.log("Resetting all settings to default...");
            await resetSettings();
        } catch (e) {
            console.error("Failed to reset settings:", e);
        }
    }

    function handleResetSystemPrompt() {
        updateDefaults({ systemPrompt: DEFAULT_SYSTEM_PROMPT });
    }

    return (
        <div className="settingsOverlay" onClick={handleCancel}>
            <div className="settingsModal" onClick={(e) => e.stopPropagation()}>
                <div className="settingsHeader">
                    <h2>Settings</h2>
                    <button className="settingsCloseBtn" onClick={handleCancel}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M18 6L6 18M6 6l12 12" />
                        </svg>
                    </button>
                </div>

                <div className="settingsContent niceScroll">
                    {/* APPEARANCE SECTION */}
                    <section className="settingsSection">
                        <h3 className="settingsSectionTitle">
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <circle cx="12" cy="12" r="3" />
                                <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                            </svg>
                            Appearance
                        </h3>

                        <div className="settingRow">
                            <label className="settingLabel">Theme</label>
                            <div className="themeSelector">
                                {(["dark", "light", "system"] as Theme[]).map((t) => (
                                    <button
                                        key={t}
                                        className={`themeOption ${localSettings.appearance.theme === t ? "active" : ""}`}
                                        onClick={() => updateAppearance({ theme: t })}
                                    >
                                        {t === "dark" && (
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                <path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" />
                                            </svg>
                                        )}
                                        {t === "light" && (
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                <circle cx="12" cy="12" r="5" />
                                                <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                                            </svg>
                                        )}
                                        {t === "system" && (
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                                                <path d="M8 21h8M12 17v4" />
                                            </svg>
                                        )}
                                        {t.charAt(0).toUpperCase() + t.slice(1)}
                                    </button>
                                ))}
                            </div>
                        </div>

                        <div className="settingRow">
                            <label className="settingLabel">Accent Color</label>
                            <div className="colorPicker">
                                <div className="colorPresets">
                                    {ACCENT_COLOR_PRESETS.map((color) => (
                                        <button
                                            key={color.hex}
                                            className={`colorSwatch ${localSettings.appearance.accentColor === color.hex ? "active" : ""}`}
                                            style={{ backgroundColor: color.hex }}
                                            onClick={() => updateAppearance({ accentColor: color.hex })}
                                            title={color.name}
                                        />
                                    ))}
                                </div>
                                <div className="customColorInput">
                                    <input
                                        type="text"
                                        value={localSettings.appearance.accentColor}
                                        onChange={(e) => {
                                            const value = e.target.value;
                                            if (value.length <= 7) {
                                                updateAppearance({ accentColor: value });
                                            }
                                        }}
                                        placeholder="#3b82f6"
                                        maxLength={7}
                                    />
                                    <div
                                        className="colorPreview"
                                        style={{ backgroundColor: localSettings.appearance.accentColor }}
                                    />
                                </div>
                            </div>
                        </div>

                        <div className="settingRow">
                            <label className="settingLabel">Font Size</label>
                            <div className="fontSizeSelector">
                                {(["small", "medium", "large"] as FontSize[]).map((size) => (
                                    <button
                                        key={size}
                                        className={`fontSizeOption ${localSettings.appearance.fontSize === size ? "active" : ""}`}
                                        onClick={() => updateAppearance({ fontSize: size })}
                                    >
                                        <span className={`fontSizePreview ${size}`}>Aa</span>
                                        {size.charAt(0).toUpperCase() + size.slice(1)}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </section>

                    {/* DEFAULTS SECTION */}
                    <section className="settingsSection">
                        <h3 className="settingsSectionTitle">
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <path d="M12 20h9M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
                            </svg>
                            Defaults
                        </h3>

                        <div className="settingRow">
                            <label className="settingLabel">Default Model</label>
                            <select
                                className="settingSelect"
                                value={localSettings.defaults.modelId || ""}
                                onChange={(e) => updateDefaults({ modelId: e.target.value || null })}
                            >
                                <option value="">Auto (first available)</option>
                                {models.map((model) => (
                                    <option key={model.id} value={model.id}>
                                        {model.name}
                                    </option>
                                ))}
                            </select>
                        </div>

                        <div className="settingRow vertical">
                            <div className="settingLabelRow">
                                <label className="settingLabel">System Prompt</label>
                                <button
                                    className="resetPromptBtn"
                                    onClick={handleResetSystemPrompt}
                                    title="Reset to default"
                                >
                                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                        <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" />
                                        <path d="M3 3v5h5" />
                                    </svg>
                                    Reset
                                </button>
                            </div>
                            <textarea
                                className="settingTextarea"
                                value={localSettings.defaults.systemPrompt}
                                onChange={(e) => updateDefaults({ systemPrompt: e.target.value })}
                                rows={6}
                            />
                        </div>
                    </section>

                    {/* BEHAVIOR SECTION */}
                    <section className="settingsSection">
                        <h3 className="settingsSectionTitle">
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <circle cx="12" cy="12" r="10" />
                                <path d="M12 6v6l4 2" />
                            </svg>
                            Behavior
                        </h3>

                        <div className="settingRow">
                            <div className="settingInfo">
                                <label className="settingLabel">Send on Enter</label>
                                <span className="settingDescription">Press Enter to send messages (Shift+Enter for new line)</span>
                            </div>
                            <label className="toggle">
                                <input
                                    type="checkbox"
                                    checked={localSettings.behavior.sendOnEnter}
                                    onChange={(e) => updateBehavior({ sendOnEnter: e.target.checked })}
                                />
                                <span className="toggleSlider"></span>
                            </label>
                        </div>

                        <div className="settingRow">
                            <div className="settingInfo">
                                <label className="settingLabel">Enable Streaming</label>
                                <span className="settingDescription">Show responses as they're generated</span>
                            </div>
                            <label className="toggle">
                                <input
                                    type="checkbox"
                                    checked={localSettings.behavior.streamingEnabled}
                                    onChange={(e) => updateBehavior({ streamingEnabled: e.target.checked })}
                                />
                                <span className="toggleSlider"></span>
                            </label>
                        </div>

                        <div className="settingRow vertical">
                            <div className="settingLabelRow">
                                <label className="settingLabel">Context Length</label>
                                <span className="contextValue">{localSettings.behavior.contextLength.toLocaleString()} tokens</span>
                            </div>
                            <span className="settingDescription">Total context window - how much conversation history the model can see</span>
                            <input
                                type="range"
                                className="settingSlider"
                                min={512}
                                max={32768}
                                step={512}
                                value={localSettings.behavior.contextLength}
                                onChange={(e) => updateBehavior({ contextLength: parseInt(e.target.value) })}
                            />
                            <div className="sliderLabels">
                                <span>512</span>
                                <span>32,768</span>
                            </div>
                        </div>

                        <div className="settingRow vertical">
                            <div className="settingLabelRow">
                                <label className="settingLabel">Max Tokens</label>
                                <span className="contextValue">{localSettings.behavior.maxTokens.toLocaleString()} tokens</span>
                            </div>
                            <span className="settingDescription">Maximum tokens per response - how long each reply can be</span>
                            <input
                                type="range"
                                className="settingSlider"
                                min={256}
                                max={16384}
                                step={256}
                                value={localSettings.behavior.maxTokens}
                                onChange={(e) => updateBehavior({ maxTokens: parseInt(e.target.value) })}
                            />
                            <div className="sliderLabels">
                                <span>256</span>
                                <span>16,384</span>
                            </div>
                        </div>
                    </section>
                </div>

                <div className="settingsFooter">
                    <button className="resetAllBtn" onClick={handleResetAll}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M3 12a9 9 0 109-9 9.75 9.75 0 00-6.74 2.74L3 8" />
                            <path d="M3 3v5h5" />
                        </svg>
                        Reset All
                    </button>
                    <div className="settingsFooterActions">
                        <button className="cancelBtn" onClick={handleCancel}>
                            Cancel
                        </button>
                        <button
                            className={`saveBtn ${hasChanges ? "active" : ""}`}
                            onClick={handleSave}
                            disabled={!hasChanges}
                        >
                            Save Changes
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}

// Apply theme preview without persisting
function applyThemePreview(settings: AppSettings): void {
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

    // Apply accent color
    document.documentElement.style.setProperty("--color-accent", accentColor);

    const rgb = hexToRgb(accentColor);
    if (rgb) {
        // Store RGB values
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

        // Even darker
        const darkerHex2 = rgbToHex(
            Math.max(0, rgb.r - 50),
            Math.max(0, rgb.g - 50),
            Math.max(0, rgb.b - 50)
        );
        document.documentElement.style.setProperty("--color-accent-darker", darkerHex2);

        // Pre-computed rgba values
        document.documentElement.style.setProperty("--color-accent-5", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.05)`);
        document.documentElement.style.setProperty("--color-accent-10", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.1)`);
        document.documentElement.style.setProperty("--color-accent-15", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.15)`);
        document.documentElement.style.setProperty("--color-accent-20", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.2)`);
        document.documentElement.style.setProperty("--color-accent-25", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.25)`);
        document.documentElement.style.setProperty("--color-accent-30", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
        document.documentElement.style.setProperty("--color-accent-40", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.4)`);
        document.documentElement.style.setProperty("--color-accent-50", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`);
        document.documentElement.style.setProperty("--color-accent-60", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.6)`);

        // Border colors
        document.documentElement.style.setProperty("--color-border", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.1)`);
        document.documentElement.style.setProperty("--color-border-accent", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
        document.documentElement.style.setProperty("--color-border-accent-hover", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`);

        // Glow colors
        document.documentElement.style.setProperty("--color-glow", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.4)`);
        document.documentElement.style.setProperty("--color-glow-strong", `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.6)`);
    }
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
