use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Entry {
    blud: String,
    lol: String,
    time: String,
}

struct AppState {
    http: reqwest::Client,
    base: String,
}

fn api_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

#[tauri::command]
async fn get_messages(
    state: tauri::State<'_, Arc<AppState>>,
    since: Option<String>,
) -> Result<Vec<Entry>, String> {
    let url = api_url(&state.base, "/api/diddy_chat");
    let mut req = state.http.get(&url);
    let since = since.unwrap_or_default();
    if !since.is_empty() {
        req = req.json(&serde_json::json!({ "since": since }));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let messages: Vec<Entry> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(messages)
}

#[tauri::command]
async fn post_message(
    state: tauri::State<'_, Arc<AppState>>,
    blud: Option<String>,
    lol: Option<String>,
) -> Result<(), String> {
    let blud = blud.unwrap_or_default();
    let lol = lol.unwrap_or_default();

    if lol.is_empty() {
        return Err("uh nuh".into());
    }
    if blud.len() > 1000 || lol.len() > 1000 {
        return Err("too long".into());
    }

    let url = api_url(&state.base, "/api/diddy_chat");
    let resp = state
        .http
        .post(&url)
        .json(&serde_json::json!({ "blud": blud, "lol": lol }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(())
}

#[tauri::command]
async fn get_local_messages(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Entry>, String> {
    let rows = sqlx::query("SELECT blud, lol, created_at FROM chat ORDER BY created_at")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    rows.into_iter()
        .map(|row| {
            Ok(Entry {
                blud: row.try_get("blud").map_err(|e| e.to_string())?,
                lol: row.try_get("lol").map_err(|e| e.to_string())?,
                time: row
                    .try_get::<i64, _>("created_at")
                    .map_err(|e| e.to_string())?
                    .to_string(),
            })
        })
        .collect()
}

#[tauri::command]
async fn append_local_messages(
    pool: tauri::State<'_, SqlitePool>,
    messages: Vec<Entry>,
) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let mut inserted = 0u64;

    for message in messages {
        let created_at = message.time.parse::<i64>().map_err(|e| e.to_string())?;
        let result =
            sqlx::query("INSERT OR IGNORE INTO chat (blud, lol, created_at) VALUES (?, ?, ?)")
                .bind(message.blud)
                .bind(message.lol)
                .bind(created_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        inserted += result.rows_affected();
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(inserted)
}

#[tauri::command]
async fn set_settings(
    pool: tauri::State<'_, SqlitePool>,
    rizz: String,
    shit: String,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO settings (rizz, shit) VALUES (?, ?)
         ON CONFLICT(rizz) DO UPDATE SET shit = excluded.shit",
    )
    .bind(rizz)
    .bind(shit)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_settings(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<HashMap<String, String>, String> {
    let rows = sqlx::query("SELECT rizz, shit FROM settings")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let mut settings = HashMap::new();
    for row in rows {
        settings.insert(
            row.try_get("rizz").map_err(|e| e.to_string())?,
            row.try_get("shit").map_err(|e| e.to_string())?,
        );
    }
    Ok(settings)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base = "https://impostorboy.ru".to_string();
    let state = Arc::new(AppState {
        http: reqwest::Client::new(),
        base,
    });

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_messages,
            post_message,
            get_local_messages,
            append_local_messages,
            set_settings,
            get_settings
        ])
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("ass.db");
            let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))?
                .create_if_missing(true);

            let pool = tauri::async_runtime::block_on(async {
                let pool = SqlitePool::connect_with(options).await?;

                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS chat (
                        blud TEXT NOT NULL,
                        lol TEXT NOT NULL,
                        created_at BIGINT NOT NULL UNIQUE
                    )",
                )
                .execute(&pool)
                .await?;

                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS settings (
                        rizz TEXT NOT NULL UNIQUE,
                        shit TEXT NOT NULL
                    )",
                )
                .execute(&pool)
                .await?;

                Ok::<SqlitePool, Box<dyn std::error::Error>>(pool)
            })?;

            app.manage(pool);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
