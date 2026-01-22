// src/hooks/useFileUpload.ts

import { useState, useRef, useCallback, useEffect } from "react";
import * as pdfjsLib from "pdfjs-dist";
import pdfjsWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import mammoth from "mammoth";
import * as XLSX from "xlsx";
import { ImageAttachment, FileAttachment } from "../types/chat";
import { getFileCategory, fileToBase64, fileToText } from "../utils/files";
import { uid } from "../utils/format";

// Set up PDF.js worker using local bundle (works in Tauri without network)
pdfjsLib.GlobalWorkerOptions.workerSrc = pdfjsWorker;

interface UseFileUploadReturn {
    pendingImages: ImageAttachment[];
    pendingFiles: FileAttachment[];
    fileInputRef: React.RefObject<HTMLInputElement | null>;
    handleFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => Promise<void>;
    removePendingImage: (id: string) => void;
    removePendingFile: (id: string) => void;
    clearPending: () => void;
    consumePending: () => { images: ImageAttachment[]; files: FileAttachment[] };
}

// Document parsing functions
async function parsePdf(file: File): Promise<string> {
    const arrayBuffer = await file.arrayBuffer();
    const pdf = await pdfjsLib.getDocument({ data: arrayBuffer }).promise;
    const textParts: string[] = [];

    for (let i = 1; i <= pdf.numPages; i++) {
        const page = await pdf.getPage(i);
        const textContent = await page.getTextContent();
        const pageText = textContent.items
            .map((item: any) => item.str)
            .join(" ");
        textParts.push(`[Page ${i}]\n${pageText}`);
    }

    return textParts.join("\n\n");
}

async function parseDocx(file: File): Promise<string> {
    const arrayBuffer = await file.arrayBuffer();
    const result = await mammoth.extractRawText({ arrayBuffer });
    return result.value;
}

async function parseExcel(file: File): Promise<string> {
    const arrayBuffer = await file.arrayBuffer();
    const workbook = XLSX.read(arrayBuffer, { type: "array" });
    const textParts: string[] = [];

    for (const sheetName of workbook.SheetNames) {
        const sheet = workbook.Sheets[sheetName];
        const csv = XLSX.utils.sheet_to_csv(sheet);
        textParts.push(`[Sheet: ${sheetName}]\n${csv}`);
    }

    return textParts.join("\n\n");
}

export function useFileUpload(): UseFileUploadReturn {
    const [pendingImages, setPendingImages] = useState<ImageAttachment[]>([]);
    const [pendingFiles, setPendingFiles] = useState<FileAttachment[]>([]);
    const fileInputRef = useRef<HTMLInputElement>(null);

    const removePendingImage = useCallback((id: string) => {
        setPendingImages((prev) => prev.filter((img) => img.id !== id));
    }, []);

    const removePendingFile = useCallback((id: string) => {
        setPendingFiles((prev) => prev.filter((f) => f.id !== id));
    }, []);

    const clearPending = useCallback(() => {
        setPendingImages([]);
        setPendingFiles([]);
    }, []);

    const consumePending = useCallback(() => {
        const images = [...pendingImages];
        const files = [...pendingFiles];
        setPendingImages([]);
        setPendingFiles([]);
        return { images, files };
    }, [pendingImages, pendingFiles]);

    const handleFileSelect = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
        const files = e.target.files;
        if (!files) return;

        const unsupportedFiles: string[] = [];

        for (const file of Array.from(files)) {
            const category = getFileCategory(file.name, file.type);

            if (category.type === "unsupported") {
                unsupportedFiles.push(file.name);
                continue;
            }

            if (category.type === "image") {
                if (file.size > 10 * 1024 * 1024) {
                    console.warn("Image too large, skipping:", file.name);
                    continue;
                }
                const base64 = await fileToBase64(file);
                setPendingImages((prev) => [
                    ...prev,
                    {
                        id: uid(),
                        base64,
                        previewUrl: `data:${file.type};base64,${base64}`,
                    },
                ]);
            } else if (category.type === "document") {
                // Parse document files (PDF, DOCX, Excel)
                if (file.size > 50 * 1024 * 1024) {
                    console.warn("Document too large, skipping:", file.name);
                    continue;
                }

                try {
                    let content: string;

                    if (category.docType === "pdf") {
                        content = await parsePdf(file);
                    } else if (category.docType === "docx") {
                        content = await parseDocx(file);
                    } else if (category.docType === "excel") {
                        content = await parseExcel(file);
                    } else {
                        console.warn("Unknown document type:", file.name);
                        continue;
                    }

                    setPendingFiles((prev) => [
                        ...prev,
                        {
                            id: uid(),
                            name: file.name,
                            type: "document",
                            content,
                        },
                    ]);
                } catch (err) {
                    console.error("Failed to parse document:", file.name, err);
                    alert(`Failed to parse ${file.name}. The file may be corrupted or password-protected.`);
                }
            } else {
                // Text or code files
                if (file.size > 5 * 1024 * 1024) {
                    console.warn("File too large, skipping:", file.name);
                    continue;
                }

                try {
                    const content = await fileToText(file);
                    setPendingFiles((prev) => [
                        ...prev,
                        {
                            id: uid(),
                            name: file.name,
                            type: category.type as "text" | "code",
                            content,
                            language: category.language,
                        },
                    ]);
                } catch (err) {
                    console.error("Failed to read file:", file.name, err);
                }
            }
        }

        // Show warning for unsupported files
        if (unsupportedFiles.length > 0) {
            alert(`The following files are not supported:\n${unsupportedFiles.join("\n")}\n\nSupported: images, PDFs, Word docs (.docx), Excel files, text files, and code files.\n\nNote: Old .doc format is not supported, please convert to .docx.`);
        }

        // Reset file input
        if (fileInputRef.current) {
            fileInputRef.current.value = "";
        }
    }, []);

    // Global paste handler for images
    useEffect(() => {
        const handlePaste = (event: ClipboardEvent) => {
            const items = event.clipboardData?.items;
            if (!items) return;

            for (const item of items) {
                if (item.type.startsWith("image")) {
                    const file = item.getAsFile();
                    if (!file) continue;

                    event.preventDefault();

                    fileToBase64(file).then((base64) => {
                        setPendingImages((prev) => [
                            ...prev,
                            {
                                id: uid(),
                                base64,
                                previewUrl: `data:${file.type};base64,${base64}`,
                            },
                        ]);
                    });
                }
            }
        };

        window.addEventListener("paste", handlePaste);

        return () => {
            window.removeEventListener("paste", handlePaste);
        };
    }, []);

    return {
        pendingImages,
        pendingFiles,
        fileInputRef,
        handleFileSelect,
        removePendingImage,
        removePendingFile,
        clearPending,
        consumePending,
    };
}
