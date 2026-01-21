// src-tauri/src/server.rs

use std::time::Duration;

pub async fn wait_for_server_ready(url: &str, timeout_secs: u64) -> Result<(), String> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/health", url);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err("Server startup timeout".to_string());
        }

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}
