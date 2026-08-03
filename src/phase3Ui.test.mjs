import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function readSource(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8").replace(/\r\n/g, "\n");
}

const appCss = readSource("./App.css");
const mainView = readSource("./MainView.tsx");
const settingsView = readSource("./SettingsView.tsx");
const widgetView = readSource("./WidgetView.tsx");
const voiceWave = readSource("./components/VoiceWave.tsx");
const historyBackend = readSource("../src-tauri/src/history.rs");
const tauriBackend = readSource("../src-tauri/src/lib.rs");

test("clear-history dialog traps focus and isolates the background", () => {
  assert.match(mainView, /cancelButtonRef\.current\?\.focus\(\)/);
  assert.match(mainView, /event\.key !== "Tab"/);
  assert.match(mainView, /event\.key === "Escape"/);
  assert.match(mainView, /clearHistoryButton\.focus\(\)/);
  assert.match(mainView, /inert=\{isClearHistoryOpen \? true : undefined\}/);
  assert.match(mainView, /aria-hidden=\{isClearHistoryOpen \? true : undefined\}/);
});

test("hotkey capture has real labels, instructions and a polite announcement", () => {
  assert.match(settingsView, /<label htmlFor="recording-hotkey"/);
  assert.match(settingsView, /<label htmlFor="repaste-hotkey"/);
  assert.match(settingsView, /id="hotkey-capture-status"[^>]*aria-live="polite"/);
  assert.match(settingsView, /Press Escape to cancel capture/);
  assert.match(settingsView, /e\.button === 1 \|\| e\.button === 3 \|\| e\.button === 4/);
  assert.match(settingsView, /recording-hotkey-help" className="text-\[11px\] leading-relaxed text-slate-400"/);
  assert.match(settingsView, /repaste-hotkey-help" className="text-\[11px\] leading-relaxed text-slate-400"/);
});

test("dictation states use one polite region while urgent failures use alerts", () => {
  assert.match(mainView, /role="status" aria-live="polite"/);
  assert.match(mainView, /role="alert"[^]*aria-atomic="true"/);
  assert.match(mainView, /showStageHint = status !== "error"/);
  assert.match(mainView, /Pasted to your app\./);
  assert.match(mainView, /Ready for paste-back\./);
  assert.match(widgetView, /role="status" aria-live="polite"/);
  assert.match(widgetView, /role=\{showError \? "alert" : undefined\}/);
});

test("compact destructive and history controls keep 24px hit areas and durable undo", () => {
  assert.match(mainView, /flex size-6 items-center justify-center rounded/);
  assert.match(settingsView, /flex size-6 self-end/);
  assert.match(mainView, /actionLabel: "Undo"/);
  assert.match(mainView, /invoke\("restore_history_item", \{ item \}\)/);
  assert.match(historyBackend, /pub fn restore\(&self, mut item: HistoryItem\)/);
  assert.match(tauriBackend, /restore_history_item/);
});

test("cursor polling is single-flight and reduced motion stays targeted", () => {
  assert.match(mainView, /let cursorSyncInFlight = false/);
  assert.match(mainView, /if \(cursorSyncInFlight\)/);
  assert.match(mainView, /Pointer enter\/leave cannot wake a transparent click-through WebView/);
  assert.doesNotMatch(appCss, /\*,\s*\n\s*\*::before,\s*\n\s*\*::after[^]*prefers-reduced-motion/);
  assert.match(appCss, /\.widget-highlight \{\s*animation: widget-highlight-reduced/);
  assert.match(voiceWave, /ease-\[var\(--fam-ease-ease\)\]/);
  assert.doesNotMatch(voiceWave, /1\.25\)\]/);
});

test("widget points to the real settings path without adding a new widget action", () => {
  assert.match(widgetView, /Tray menu → Settings\./);
  assert.doesNotMatch(widgetView, /aria-label="Settings"/);
});
