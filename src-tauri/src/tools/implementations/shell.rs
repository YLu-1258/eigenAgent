// src-tauri/src/tools/implementations/shell.rs

use std::process::Command;
use std::time::Duration;

use crate::tools::types::{ToolCallRequest, ToolCallResult};

const TIMEOUT_SECS: u64 = 30;
const MAX_OUTPUT_SIZE: usize = 100_000; // 100KB max output

pub async fn execute(request: &ToolCallRequest) -> ToolCallResult {
    let command = match request.arguments.get("command").and_then(|v| v.as_str()) {
        Some(cmd) => cmd,
        None => {
            return ToolCallResult::error(
                request.call_id.clone(),
                "Missing required parameter: command".to_string(),
            )
        }
    };

    // Security check: block dangerous commands
    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf ~",
        "mkfs",
        "dd if=",
        ":(){:|:&};:",  // Fork bomb
        "chmod -R 777 /",
        "chown -R",
        "> /dev/sd",
        "curl | sh",
        "curl | bash",
        "wget | sh",
        "wget | bash",
    ];

    let cmd_lower = command.to_lowercase();
    for pattern in &dangerous_patterns {
        if cmd_lower.contains(pattern) {
            return ToolCallResult::error(
                request.call_id.clone(),
                format!("Blocked potentially dangerous command pattern: {}", pattern),
            );
        }
    }

    // Execute command with timeout
    let call_id = request.call_id.clone();
    let command_owned = command.to_string();

    // Use tokio's blocking spawn for the sync command execution
    let result = tokio::task::spawn_blocking(move || {
        execute_command_sync(&command_owned, TIMEOUT_SECS)
    })
    .await;

    match result {
        Ok(cmd_result) => match cmd_result {
            Ok((stdout, stderr, exit_code)) => {
                let mut output = String::new();

                if !stdout.is_empty() {
                    output.push_str(&stdout);
                }

                if !stderr.is_empty() {
                    if !output.is_empty() {
                        output.push_str("\n\n--- stderr ---\n");
                    }
                    output.push_str(&stderr);
                }

                if output.is_empty() {
                    output = format!("Command completed with exit code {}", exit_code);
                } else if exit_code != 0 {
                    output.push_str(&format!("\n\nExit code: {}", exit_code));
                }

                // Truncate if too large
                if output.len() > MAX_OUTPUT_SIZE {
                    output.truncate(MAX_OUTPUT_SIZE);
                    output.push_str("\n\n... (output truncated)");
                }

                if exit_code == 0 {
                    ToolCallResult::success(call_id, output)
                } else {
                    // Still return success but include exit code in output
                    ToolCallResult::success(call_id, output)
                }
            }
            Err(e) => ToolCallResult::error(call_id, e),
        },
        Err(e) => ToolCallResult::error(call_id, format!("Task execution failed: {}", e)),
    }
}

fn execute_command_sync(command: &str, timeout_secs: u64) -> Result<(String, String, i32), String> {
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };

    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut child = Command::new(shell)
        .arg(shell_arg)
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    // Wait with timeout
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        use std::io::Read;
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        use std::io::Read;
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                return Ok((stdout, stderr, status.code().unwrap_or(-1)));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(format!(
                        "Command timed out after {} seconds",
                        timeout_secs
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Error waiting for command: {}", e));
            }
        }
    }
}
