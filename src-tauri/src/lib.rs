use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_background_service::{init_with_service, BackgroundService, ServiceContext, ServiceError};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;

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

async fn insert_local_message(pool: &SqlitePool, message: &Entry) -> Result<bool, String> {
    let created_at = message.time.parse::<i64>().map_err(|e| e.to_string())?;
    let result =
        sqlx::query("INSERT OR IGNORE INTO chat (blud, lol, created_at) VALUES (?, ?, ?)")
            .bind(&message.blud)
            .bind(&message.lol)
            .bind(created_at)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(result.rows_affected() > 0)
}

async fn max_local_created_at(pool: &SqlitePool) -> Result<Option<i64>, String> {
    let row = sqlx::query("SELECT MAX(created_at) AS max_created_at FROM chat")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    row.try_get::<Option<i64>, _>("max_created_at")
        .map_err(|e| e.to_string())
}

async fn fetch_remote_messages(
    http: &reqwest::Client,
    base: &str,
    since: Option<&str>,
) -> Result<Vec<Entry>, String> {
    let url = api_url(base, "/api/diddy_chat");
    let mut req = http.get(&url);
    if let Some(since) = since.filter(|s| !s.is_empty()) {
        req = req.json(&serde_json::json!({ "since": since }));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

async fn local_nick(pool: &SqlitePool) -> Result<String, String> {
    let row = sqlx::query("SELECT shit FROM settings WHERE rizz = ?")
        .bind("nick")
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match row {
        Some(row) => row.try_get::<String, _>("shit").map_err(|e| e.to_string())?,
        None => String::new(),
    })
}

fn should_notify(entry: &Entry, nick: &str, is_foreground: bool) -> bool {
    if is_foreground {
        return false;
    }
    let nick = nick.trim();
    if !nick.is_empty() && entry.blud == nick {
        return false;
    }
    true
}

async fn catch_up_messages<R: tauri::Runtime>(ctx: &ServiceContext<R>) -> Result<(), String> {
    let pool = ctx.app.state::<SqlitePool>().inner().clone();
    let state = ctx.app.state::<Arc<AppState>>().inner().clone();
    let foreground = ctx.app.state::<Arc<AtomicBool>>().inner().clone();

    let since = max_local_created_at(&pool)
        .await?
        .map(|v| v.to_string());
    let remote = fetch_remote_messages(&state.http, &state.base, since.as_deref()).await?;
    let nick = local_nick(&pool).await.unwrap_or_default();
    let is_foreground = foreground.load(Ordering::SeqCst);

    for entry in remote {
        let inserted = insert_local_message(&pool, &entry).await.unwrap_or(false);
        if inserted {
            let _ = ctx.app.emit("chat://message", &entry);
            if should_notify(&entry, &nick, is_foreground) {
                let title = if entry.blud.is_empty() { "???" } else { &entry.blud };
                if let Err(e) = ctx
                    .app
                    .notification()
                    .builder()
                    .channel_id("chat_messages")
                    .title(title)
                    .body(&entry.lol)
                    .show()
                {
                    eprintln!("[chat-bg] notification failed: {e}");
                }
            }
        }
    }

    Ok(())
}

struct ChatService;

impl ChatService {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl<R: tauri::Runtime> BackgroundService<R> for ChatService {
    async fn init(&mut self, _ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        let ws_url = "wss://impostorboy.ru/ws".to_string();
        eprintln!("[chat-bg] service started");

        loop {
            if ctx.shutdown.is_cancelled() {
                break;
            }

            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    eprintln!("[chat-bg] ws connected");
                    if let Err(e) = catch_up_messages(ctx).await {
                        eprintln!("[chat-bg] catch-up failed: {e}");
                    }
                    let (_write, mut read) = ws_stream.split();
                    loop {
                        tokio::select! {
                            _ = ctx.shutdown.cancelled() => {
                                eprintln!("[chat-bg] shutdown");
                                return Ok(());
                            }
                            msg = read.next() => {
                                let Some(msg) = msg else {
                                    eprintln!("[chat-bg] ws stream ended");
                                    break;
                                };
                                let msg = match msg {
                                    Ok(msg) => msg,
                                    Err(e) => {
                                        eprintln!("[chat-bg] ws read error: {e}");
                                        break;
                                    }
                                };
                                if !msg.is_text() {
                                    continue;
                                }
                                let Ok(text) = msg.to_text() else { continue; };
                                let Ok(entry) = serde_json::from_str::<Entry>(text) else {
                                    eprintln!("[chat-bg] bad ws payload: {text}");
                                    continue;
                                };

                                let pool = ctx.app.state::<SqlitePool>().inner().clone();
                                let inserted = insert_local_message(&pool, &entry).await.unwrap_or(false);
                                eprintln!("[chat-bg] msg inserted={inserted} time={}", entry.time);
                                if inserted {
                                    let _ = ctx.app.emit("chat://message", &entry);
                                    let foreground = ctx.app.state::<Arc<AtomicBool>>().inner().clone();
                                    let nick = local_nick(&pool).await.unwrap_or_default();
                                    let is_foreground = foreground.load(Ordering::SeqCst);
                                    if should_notify(&entry, &nick, is_foreground) {
                                        let title = if entry.blud.is_empty() { "???" } else { &entry.blud };
                                        if let Err(e) = ctx
                                            .app
                                            .notification()
                                            .builder()
                                            .channel_id("chat_messages")
                                            .title(title)
                                            .body(&entry.lol)
                                            .show()
                                        {
                                            eprintln!("[chat-bg] notification failed: {e}");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[chat-bg] ws connect failed: {e}");
                }
            }

            tokio::select! {
                _ = ctx.shutdown.cancelled() => break,
                _ = sleep(Duration::from_secs(3)) => {}
            }
        }

        Ok(())
    }
}

#[tauri::command]
async fn get_messages(
    state: tauri::State<'_, Arc<AppState>>,
    since: Option<String>,
) -> Result<Vec<Entry>, String> {
    fetch_remote_messages(&state.http, &state.base, since.as_deref()).await
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

#[tauri::command]
fn set_foreground(foreground: tauri::State<'_, Arc<AtomicBool>>, active: bool) {
    foreground.store(active, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base = "https://impostorboy.ru".to_string();
    let state = Arc::new(AppState {
        http: reqwest::Client::new(),
        base,
    });

    let foreground = Arc::new(AtomicBool::new(true));

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(init_with_service(|| ChatService::new()))
        .manage(state)
        .manage(foreground)
        .invoke_handler(tauri::generate_handler![
            get_messages,
            post_message,
            get_local_messages,
            append_local_messages,
            set_settings,
            get_settings,
            set_foreground
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

            #[cfg(mobile)]
            {
                let channel = tauri_plugin_notification::Channel::builder(
                    "chat_messages",
                    "Chat messages",
                )
                .description("New chat messages")
                .build();
                if let Err(e) = app.notification().create_channel(channel) {
                    eprintln!("[chat-bg] create notification channel failed: {e}");
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat (
                blud TEXT NOT NULL,
                lol TEXT NOT NULL,
                created_at BIGINT NOT NULL UNIQUE
            )",
        )
        .execute(&pool)
        .await
        .expect("chat table");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                rizz TEXT NOT NULL UNIQUE,
                shit TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("settings table");
        pool
    }

    fn entry(blud: &str, lol: &str, time: &str) -> Entry {
        Entry {
            blud: blud.to_string(),
            lol: lol.to_string(),
            time: time.to_string(),
        }
    }

    #[test]
    fn should_notify_skips_own_message() {
        let e = entry("alice", "hi", "1");
        assert!(!should_notify(&e, "alice", false));
    }

    #[test]
    fn should_notify_allows_other_message() {
        let e = entry("bob", "hi", "1");
        assert!(should_notify(&e, "alice", false));
    }

    #[test]
    fn should_notify_skips_when_foreground() {
        let e = entry("bob", "hi", "1");
        assert!(!should_notify(&e, "alice", true));
    }

    #[test]
    fn should_notify_allows_empty_nick() {
        let e = entry("bob", "hi", "1");
        assert!(should_notify(&e, "", false));
    }

    #[tokio::test]
    async fn insert_local_message_dedups_by_created_at() {
        let pool = test_pool().await;
        let e = entry("a", "b", "123");
        assert!(insert_local_message(&pool, &e).await.unwrap());
        assert!(!insert_local_message(&pool, &e).await.unwrap());

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn max_local_created_at_returns_none_when_empty() {
        let pool = test_pool().await;
        assert_eq!(max_local_created_at(&pool).await.unwrap(), None);
    }

    #[tokio::test]
    async fn max_local_created_at_returns_max() {
        let pool = test_pool().await;
        insert_local_message(&pool, &entry("a", "1", "10"))
            .await
            .unwrap();
        insert_local_message(&pool, &entry("a", "2", "20"))
            .await
            .unwrap();
        insert_local_message(&pool, &entry("a", "3", "15"))
            .await
            .unwrap();
        assert_eq!(max_local_created_at(&pool).await.unwrap(), Some(20));
    }

    #[tokio::test]
    async fn local_nick_reads_setting() {
        let pool = test_pool().await;
        assert_eq!(local_nick(&pool).await.unwrap(), "");
        sqlx::query("INSERT INTO settings (rizz, shit) VALUES ('nick', 'alice')")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(local_nick(&pool).await.unwrap(), "alice");
    }
}
