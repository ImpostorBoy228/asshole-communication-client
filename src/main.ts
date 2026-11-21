import { invoke } from "@tauri-apps/api/core";

interface Entry {
  blud: string;
  lol: string;
  time: string;
}

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

async function loadMessages(): Promise<void> {
  const messages = await invoke<Entry[]>("get_messages");
  const container = document.getElementById("messages")!;
  container.replaceChildren();
  for (const m of messages) {
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
    container.appendChild(div);
  }
}

document.getElementById("messageForm")!.addEventListener("submit", async (e) => {
  e.preventDefault();
  const blud = (document.getElementById("blud") as HTMLInputElement).value;
  const lol = (document.getElementById("lol") as HTMLInputElement).value;
  await invoke("post_message", { blud, lol });
  (document.getElementById("lol") as HTMLInputElement).value = "";
  await loadMessages();
});

loadMessages();