// src-tauri/src/db.rs

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use tauri::AppHandle;
use tauri::Manager;

pub fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as i64
}

pub fn open_db(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_millis(2000))
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn init_db(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS conversations (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            summary     TEXT NOT NULL DEFAULT '',
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            thinking        TEXT NOT NULL DEFAULT '',
            images          TEXT NOT NULL DEFAULT '[]',
            created_at      INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv_created
            ON messages(conversation_id, created_at);
        "#,
    )
    .map_err(|e| e.to_string())?;

    // Migration: add thinking column if missing
    let has_thinking: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'thinking'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_thinking {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN thinking TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // Migration: add images column if missing
    let has_images: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'images'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_images {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN images TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // Migration: add duration_ms column if missing
    let has_duration: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name = 'duration_ms'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_duration {
        conn.execute("ALTER TABLE messages ADD COLUMN duration_ms INTEGER", [])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn insert_message(
    conn: &Connection,
    chat_id: &str,
    role: &str,
    content: &str,
    thinking: &str,
    images: &[String],
    duration_ms: Option<i64>,
) -> Result<(), String> {
    let now = unix_ms();
    let msg_id = uuid::Uuid::new_v4().to_string();
    let images_json = serde_json::to_string(images).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, thinking, images, created_at, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            msg_id,
            chat_id,
            role,
            content,
            thinking,
            images_json,
            now,
            duration_ms,
        ],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn resolve_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("eigenAgent.sqlite3"))
}
