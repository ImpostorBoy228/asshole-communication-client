NEVER USE write tool
USE INSTEAD edit tool

# Project: asshole-communication

## Overview
Tauri 2 desktop app for a small chat ("Asshole Communication"). Frontend is vanilla TypeScript + HTML/CSS (Tailwind CSS 4 + basecoat-css). Backend is Rust Tauri commands with a local SQLite cache. The app syncs messages with a remote API at `https://impostorboy.ru/api/diddy_chat`.

## Stack
- Frontend: Vite 8 (root `src`), TypeScript ~6.0, Tailwind CSS 4 (`@tailwindcss/vite`), basecoat-css. Two entry points: `index.html`, `settings.html`.
- Backend: Rust 2024, Tauri 2, sqlx (sqlite), reqwest (rustls-tls).
- Package manager: bun (`bun.lock`; tauri `beforeDevCommand` is `bun run dev`).
- Build: `bun run build` (= `tsc && vite build`), `bun run tauri dev` / `bun run tauri build`.
- No test suite configured.

## Structure
- `src/` — frontend sources: `main.ts`, `settings.ts`, `styles.css`, HTML, images.
- `src-tauri/` — Rust backend: `src/lib.rs` (commands + setup), `src/main.rs`, `tauri.conf.json`, `capabilities/`.
- `dist/` — Vite build output (frontendDist).
- Local DB: `{app_data_dir}/ass.db` (SQLite) with tables `chat`, `settings`.

## Tauri commands (src-tauri/src/lib.rs)
- `get_messages(since?: String)` — GET remote messages (optional `since`).
- `post_message(blud?, lol?)` — POST a message; validates non-empty `lol`, max 1000 chars.
- `get_local_messages()` — read cached messages from SQLite.
- `append_local_messages(messages)` — upsert cached messages (`INSERT OR IGNORE`).
- `get_settings()` — returns `HashMap<String,String>`.
- `set_settings(rizz, shit)` — upsert a setting by key `rizz` with value `shit`.

## Data model
- `Entry { blud: String, lol: String, time: String }` where `time` is nanoseconds as a string.
- DB `chat(blud TEXT, lol TEXT, created_at BIGINT NOT NULL UNIQUE)`.
- DB `settings(rizz TEXT NOT NULL UNIQUE, shit TEXT NOT NULL)`.
- Nick is stored under settings key `"nick"`.

## Remote API
- Base: `https://impostorboy.ru`.
- `GET /api/diddy_chat` with body `{ since }` (when provided) → `Vec<Entry>`.
- `POST /api/diddy_chat` with body `{ blud, lol }`.

## Frontend behavior
- `main.ts`: loads nick from settings; syncs messages (local cache first, then remote delta via `since`); posts messages; renders with custom ITS time formatting (`147y 21m …`).
- `settings.ts`: saves nick under key `"nick"` via `set_settings({ rizz: "nick", shit: nick })`.
- Hint text "сначала ник" when no nick set.

## Conventions / notes
- UI text is partly Russian.
- Param/field names are intentionally obfuscated (`blud`, `lol`, `rizz`, `shit`) — keep consistent across Rust and TS.
- Vite root is `src`; output `../dist`; ports 1420 (dev) / 1421 (HMR).
- CSP is null; `withGlobalTauri` enabled.
- Do not commit `src-tauri/target` or `node_modules` (ignored).
