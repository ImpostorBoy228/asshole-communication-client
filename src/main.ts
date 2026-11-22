import "basecoat-css/all";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { onBackButtonPress } from "@tauri-apps/api/app";

interface Entry {
  blud: string;
  lol: string;
  time: string;
}

const messagesEl = document.getElementById("messages")!;
const form = document.getElementById("messageForm") as HTMLFormElement;
const lolInput = document.getElementById("lol") as HTMLInputElement;
const nickHint = document.getElementById("nickHint")!;

let nick = "";
const seenTimes = new Set<string>();

function formatITS(ns: string): string {
  const totalNs = BigInt(ns);
  const Y = BigInt(147) * BigInt(86400) * BigInt(1000000000);
  const M = BigInt(21) * BigInt(86400) * BigInt(1000000000);
  const D = BigInt(86400) * BigInt(1000000000);
  const H = BigInt(3600) * BigInt(1000000000);
  const m = BigInt(60) * BigInt(1000000000);
  let r = totalNs;
  const y = r / Y;
  r %= Y;
  const mo = r / M;
  r %= M;
  const d = r / D;
  r %= D;
  const h = r / H;
  r %= H;
  const mi = r / m;
  r %= m;
  const s = Number(r) / 1e9;
  return `${y}y ${mo}m ${d}d ${String(h).padStart(2, "0")}:${String(mi).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function messageNode(m: Entry): HTMLDivElement {
  const div = document.createElement("div");
  const b = document.createElement("b");
  b.textContent = m.blud || "???";
  const txt = document.createTextNode(": ");
  const span = document.createElement("span");
  span.textContent = m.lol;
  const small = document.createElement("small");
  small.style.color = "#8c8c8c";
  small.textContent = " " + formatITS(m.time).slice(0, -10);
  div.append(b, txt, span, small);
  return div;
}

function renderMessages(messages: Entry[]): void {
  seenTimes.clear();
  for (const m of messages) seenTimes.add(m.time);
  messagesEl.replaceChildren(...messages.map(messageNode));
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function appendMessage(m: Entry): void {
  if (seenTimes.has(m.time)) return;
  seenTimes.add(m.time);
  messagesEl.appendChild(messageNode(m));
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function setHint(text: string): void {
  nickHint.textContent = text;
  nickHint.hidden = text === "";
}

async function loadNick(): Promise<void> {
  try {
    const settings = await invoke<Record<string, string>>("get_settings");
    nick = (settings["nick"] ?? "").trim();
  } catch {
    nick = "";
  }
  setHint("");
}

// Local cache first: draw what we already have, then ask the server only for
// what came after the newest cached message. An empty cache means a full fetch.
async function syncMessages(): Promise<void> {
  const local = await invoke<Entry[]>("get_local_messages");
  renderMessages(local);

  const since = local.length > 0 ? local[local.length - 1].time : undefined;
  const remote = await invoke<Entry[]>("get_messages", since ? { since } : {});
  if (remote.length === 0) {
    return;
  }

  await invoke("append_local_messages", { messages: remote });
  renderMessages(await invoke<Entry[]>("get_local_messages"));
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();

  const lol = lolInput.value.trim();
  if (lol === "") {
    return;
  }

  try {
    await invoke("post_message", { blud: nick, lol });
    lolInput.value = "";
    setHint("");
    await syncMessages();
  } catch (err) {
    setHint(String(err));
  }
});

async function startBackgroundService(): Promise<void> {
  try {
    await invoke("plugin:background-service|start", { config: {} });
    console.log("[bg] service started");
  } catch (err) {
    console.warn("[bg] service start failed", err);
  }
}

async function requestNotificationPermission(): Promise<void> {
  try {
    const res = await invoke("plugin:background-service|request_notification_permission");
    console.log("[bg] notification permission", res);
  } catch (err) {
    console.warn("[bg] notification permission failed", err);
  }
}

function trackForeground(): void {
  const update = () => {
    const active = !document.hidden && document.hasFocus();
    invoke("set_foreground", { active }).catch(() => {});
  };
  document.addEventListener("visibilitychange", update);
  window.addEventListener("focus", update);
  window.addEventListener("blur", update);
  update();
}

async function start(): Promise<void> {
  await loadNick();

  await listen<Entry>("chat://message", (event) => {
    console.log("[ws] message", event.payload);
    appendMessage(event.payload);
  });

  trackForeground();
  await requestNotificationPermission();
  await startBackgroundService();

  try {
    await syncMessages();
  } catch (err) {
    setHint(String(err));
  }
}

onBackButtonPress(() => {
  // Root page: hardware Back does nothing, like a native app root screen.
});

// Treat in-app navigation as replace (no browser-style history) so Back
// never bounces between pages.
document.querySelector<HTMLAnchorElement>('a[href="settings.html"]')?.addEventListener("click", (e) => {
  e.preventDefault();
  location.replace("settings.html");
});

start();
