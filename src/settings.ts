import "basecoat-css/all";
import { invoke } from "@tauri-apps/api/core";

const NICK_KEY = "nick";

const form = document.getElementById("settingsNick") as HTMLFormElement;
const nickInput = document.getElementById("nick") as HTMLInputElement;
const savedEl = document.getElementById("saved")!;
const errorEl = document.getElementById("error")!;

function showError(err: unknown): void {
  savedEl.hidden = true;
  errorEl.textContent = String(err);
  errorEl.hidden = false;
}

let savedTimer: number | undefined;

function showSaved(): void {
  errorEl.hidden = true;
  savedEl.hidden = false;
  window.clearTimeout(savedTimer);
  savedTimer = window.setTimeout(() => {
    savedEl.hidden = true;
  }, 2000);
}

async function loadSettings(): Promise<void> {
  try {
    const settings = await invoke<Record<string, string>>("get_settings");
    nickInput.value = settings[NICK_KEY] ?? "";
  } catch (err) {
    showError(err);
  }
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();

  const nick = nickInput.value.trim();
  if (nick === "") {
    nickInput.focus();
    return;
  }

  try {
    await invoke("set_settings", { rizz: NICK_KEY, shit: nick });
    nickInput.value = nick;
    showSaved();
  } catch (err) {
    showError(err);
  }
});

loadSettings();
