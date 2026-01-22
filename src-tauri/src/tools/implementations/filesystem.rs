// src-tauri/src/tools/implementations/filesystem.rs

use std::fs;
use std::path::Path;

use crate::tools::types::{ToolCallRequest, ToolCallResult};

pub async fn execute(request: &ToolCallRequest) -> ToolCallResult {
    let operation = match request.arguments.get("operation").and_then(|v| v.as_str()) {
        Some(op) => op,
        None => {
            return ToolCallResult::error(
                request.call_id.clone(),
                "Missing required parameter: operation".to_string(),
            )
        }
    };

    let path_str = match request.arguments.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            return ToolCallResult::error(
                request.call_id.clone(),
                "Missing required parameter: path".to_string(),
            )
        }
    };

    // Expand ~ to home directory
    let path_str = if path_str.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            path_str.replacen("~", home.to_str().unwrap_or(""), 1)
        } else {
            path_str.to_string()
        }
    } else {
        path_str.to_string()
    };

    let path = Path::new(&path_str);

    // Security check: prevent access to sensitive system directories
    let path_lower = path_str.to_lowercase();
    let forbidden_paths = [
        "/etc/passwd",
        "/etc/shadow",
        "/etc/sudoers",
        ".ssh/",
        ".gnupg/",
        ".aws/credentials",
        ".env",
    ];

    for forbidden in &forbidden_paths {
        if path_lower.contains(forbidden) {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Access denied: cannot access sensitive path '{}'", path_str),
            );
        }
    }

    match operation {
        "read" => read_file(request.call_id.clone(), path),
        "write" => {
            let content = request
                .arguments
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            write_file(request.call_id.clone(), path, content)
        }
        "list" => list_directory(request.call_id.clone(), path),
        _ => ToolCallResult::error(
            request.call_id.clone(),
            format!("Unknown operation: {}. Use 'read', 'write', or 'list'", operation),
        ),
    }
}

fn read_file(call_id: String, path: &Path) -> ToolCallResult {
    if !path.exists() {
        return ToolCallResult::error(call_id, format!("File not found: {}", path.display()));
    }

    if !path.is_file() {
        return ToolCallResult::error(call_id, format!("Not a file: {}", path.display()));
    }

    // Check file size (limit to 1MB)
    match fs::metadata(path) {
        Ok(meta) => {
            if meta.len() > 1_000_000 {
                return ToolCallResult::error(
                    call_id,
                    "File too large (>1MB). Consider reading a smaller file.".to_string(),
                );
            }
        }
        Err(e) => {
            return ToolCallResult::error(call_id, format!("Cannot read file metadata: {}", e))
        }
    }

    match fs::read_to_string(path) {
        Ok(content) => ToolCallResult::success(call_id, content),
        Err(e) => ToolCallResult::error(call_id, format!("Failed to read file: {}", e)),
    }
}

fn write_file(call_id: String, path: &Path, content: &str) -> ToolCallResult {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolCallResult::error(
                    call_id,
                    format!("Failed to create directory: {}", e),
                );
            }
        }
    }

    match fs::write(path, content) {
        Ok(()) => ToolCallResult::success(
            call_id,
            format!("Successfully wrote {} bytes to {}", content.len(), path.display()),
        ),
        Err(e) => ToolCallResult::error(call_id, format!("Failed to write file: {}", e)),
    }
}

fn list_directory(call_id: String, path: &Path) -> ToolCallResult {
    if !path.exists() {
        return ToolCallResult::error(call_id, format!("Directory not found: {}", path.display()));
    }

    if !path.is_dir() {
        return ToolCallResult::error(call_id, format!("Not a directory: {}", path.display()));
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            return ToolCallResult::error(call_id, format!("Failed to read directory: {}", e))
        }
    };

    let mut output = format!("Contents of {}:\n\n", path.display());
    let mut files: Vec<(String, bool, u64)> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push((name, is_dir, size));
    }

    // Sort: directories first, then files, alphabetically
    files.sort_by(|a, b| {
        match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        }
    });

    for (name, is_dir, size) in files {
        if is_dir {
            output.push_str(&format!("📁 {}/\n", name));
        } else {
            let size_str = format_size(size);
            output.push_str(&format!("📄 {} ({})\n", name, size_str));
        }
    }

    ToolCallResult::success(call_id, output)
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
