// src/utils/files.ts

// Code file extensions and their language identifiers
export const CODE_EXTENSIONS: Record<string, string> = {
    js: "javascript", jsx: "javascript", ts: "typescript", tsx: "typescript",
    py: "python", rb: "ruby", java: "java", c: "c", cpp: "cpp", h: "c",
    hpp: "cpp", cs: "csharp", go: "go", rs: "rust", swift: "swift",
    kt: "kotlin", scala: "scala", php: "php", sh: "bash", bash: "bash",
    zsh: "bash", fish: "fish", ps1: "powershell", sql: "sql", r: "r",
    lua: "lua", perl: "perl", hs: "haskell", ml: "ocaml", clj: "clojure",
    ex: "elixir", exs: "elixir", erl: "erlang", dart: "dart", vue: "vue",
    svelte: "svelte", css: "css", scss: "scss", sass: "sass", less: "less",
    html: "html", htm: "html", xml: "xml", json: "json", yaml: "yaml",
    yml: "yaml", toml: "toml", ini: "ini", cfg: "ini", conf: "ini",
    md: "markdown", mdx: "markdown", dockerfile: "dockerfile", makefile: "makefile",
};

export const TEXT_EXTENSIONS = ["txt", "log", "csv", "tsv", "env", "gitignore", "editorconfig"];

export type FileCategory = {
    type: "image" | "text" | "code" | "document" | "unsupported";
    language?: string;
    docType?: string;
};

export function getFileCategory(filename: string, mimeType: string): FileCategory {
    const ext = filename.split(".").pop()?.toLowerCase() || "";

    if (mimeType.startsWith("image/")) {
        return { type: "image" };
    }

    if (CODE_EXTENSIONS[ext]) {
        return { type: "code", language: CODE_EXTENSIONS[ext] };
    }

    if (TEXT_EXTENSIONS.includes(ext)) {
        return { type: "text" };
    }

    // Supported document types
    if (mimeType === "application/pdf" || ext === "pdf") {
        return { type: "document", docType: "pdf" };
    }

    if (mimeType === "application/vnd.openxmlformats-officedocument.wordprocessingml.document" || ext === "docx") {
        return { type: "document", docType: "docx" };
    }

    if (mimeType === "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" || ext === "xlsx" ||
        mimeType === "application/vnd.ms-excel" || ext === "xls") {
        return { type: "document", docType: "excel" };
    }

    // Unsupported binary formats
    if (ext === "doc") {
        return { type: "unsupported" }; // Old .doc format not supported
    }

    if (mimeType.startsWith("text/") || mimeType === "application/json" || mimeType === "application/xml") {
        return { type: "text" };
    }

    // Default to text for unknown types (may fail for binary files)
    return { type: "text" };
}

export function fileToBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
            const result = reader.result as string;
            // Remove data URL prefix
            resolve(result.split(",")[1]);
        };
        reader.onerror = reject;
        reader.readAsDataURL(file);
    });
}

export function fileToText(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
            resolve(reader.result as string);
        };
        reader.onerror = reject;
        reader.readAsText(file);
    });
}
