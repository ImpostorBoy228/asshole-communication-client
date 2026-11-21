use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base = "https://impostorboy.ru".to_string();
    let state = Arc::new(AppState {
        http: reqwest::Client::new(),
        base,
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![get_messages, post_message])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}