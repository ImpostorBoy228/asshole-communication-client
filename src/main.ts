import "basecoat-css/all";
import { invoke } from "@tauri-apps/api/core";
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
  messagesEl.replaceChildren(...messages.map(messageNode));
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

  if (nick === "") {
    setHint("сначала ник");
  } else {
    setHint("");
  }
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

  if (nick === "") {
    setHint("сначала ник");
    return;
  }

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

async function start(): Promise<void> {
  await loadNick();

  try {
    await syncMessages();
  } catch (err) {
    // "сначала ник" is more important than a network error while there is no nick
    if (nick !== "") {
      setHint(String(err));
    }
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
