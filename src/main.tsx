import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// Early theme initialization to prevent flash
(function initTheme() {
    const storedTheme = localStorage.getItem("eigenAgent-theme") as "dark" | "light" | null;
    const storedFontSize = localStorage.getItem("eigenAgent-fontSize") || "medium";
    const storedAccentColor = localStorage.getItem("eigenAgent-accentColor") || "#3b82f6";

    const theme = storedTheme || "dark";

    document.documentElement.setAttribute("data-theme", theme);
    document.documentElement.setAttribute("data-font-size", storedFontSize);
    document.body.style.margin = "0";
    document.body.style.background = theme === "dark" ? "#0a0a0a" : "#ffffff";

    // Apply accent color
    if (storedAccentColor) {
        document.documentElement.style.setProperty("--color-accent", storedAccentColor);
    }
})();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
        <App />
    </React.StrictMode>,
);
