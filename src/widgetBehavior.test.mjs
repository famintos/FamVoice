import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const widgetViewSource = readFileSync(new URL("./WidgetView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const voiceWaveSource = readFileSync(new URL("./components/VoiceWave.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

function getWidgetBranchBlock() {
  const widgetBranchIndex = mainViewSource.indexOf("if (settings?.widget_mode) {");
  assert.notEqual(widgetBranchIndex, -1, "expected widget mode branch in MainView.tsx");

  const widgetBranchEnd = mainViewSource.indexOf("\n\n  return (", widgetBranchIndex);
  assert.notEqual(widgetBranchEnd, -1, "expected widget mode branch end in MainView.tsx");

  return mainViewSource.slice(widgetBranchIndex, widgetBranchEnd);
}

function getSettingsHeaderBlock() {
  const headerClassIndex = settingsViewSource.indexOf('className="relative z-10 flex items-center justify-between px-4 py-2.5"');
  assert.notEqual(headerClassIndex, -1, "expected settings header wrapper in SettingsView.tsx");

  const headerStart = settingsViewSource.lastIndexOf("<div", headerClassIndex);
  assert.notEqual(headerStart, -1, "expected settings header wrapper start");

  return settingsViewSource.slice(headerStart, headerClassIndex + 600);
}

function getRecordTabBlock() {
  const recordTabIndex = mainViewSource.indexOf('{activeTab === "record" ? (');
  assert.notEqual(recordTabIndex, -1, "expected record tab branch in MainView.tsx");

  return mainViewSource.slice(recordTabIndex, recordTabIndex + 2600);
}

test("widget container uses manual dragging instead of native drag-region", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.equal(widgetViewSource.includes("data-tauri-drag-region"), false);
  assert.match(widgetViewSource, /className="widget-shell/);
  assert.match(widgetViewSource, /id="widget-container"/);
  assert.match(widgetViewSource, /onMouseDownCapture=\{onMouseDownCapture\}/);
  assert.match(widgetBranchBlock, /onMouseDownCapture=\{\(e\) => \{/);
  assert.match(widgetBranchBlock, /void appWindow\.startDragging\(\)\.catch/);
});

test("widget right click is ignored instead of opening settings", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.doesNotMatch(widgetBranchBlock, /void handleOpenSettings\(\)/);
  assert.doesNotMatch(widgetBranchBlock, /void handleUpdate\(\)/);
  assert.match(widgetViewSource, /onContextMenu=\{\(e\) => \{/);
  assert.match(widgetViewSource, /e\.preventDefault\(\);/);
});

test("widget drag start sets a grace period before requesting the window drag", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.match(widgetBranchBlock, /widgetDragGraceUntilRef\.current = Date\.now\(\) \+ WIDGET_DRAG_START_GRACE_MS/);
  assert.match(widgetBranchBlock, /ignoreCursorEventsRef\.current = false/);
  assert.match(widgetBranchBlock, /void appWindow\.setIgnoreCursorEvents\(false\)/);
});

test("settings window keeps drag behavior on the header only", () => {
  const headerBlock = getSettingsHeaderBlock();

  assert.doesNotMatch(settingsViewSource, /<main[^>]*data-tauri-drag-region/);
  assert.match(headerBlock, /<div data-tauri-drag-region className="relative z-10 flex items-center justify-between px-4 py-2\.5">/);
});

test("voice wave supports explicit modes and size variants", () => {
  assert.match(voiceWaveSource, /mode\?: "idle" \| "recording" \| "transcribing"/);
  assert.doesNotMatch(voiceWaveSource, /isPlaying\?: boolean/);
  assert.match(voiceWaveSource, /mode === "transcribing"/);
  assert.match(voiceWaveSource, /size = "default"/);
  assert.match(voiceWaveSource, /size\?: "default" \| "widget" \| "large"/);
  assert.match(voiceWaveSource, /const isActiveWidget = size === "widget" && mode !== "idle";/);
  assert.match(voiceWaveSource, /size === "widget"/);
  assert.match(voiceWaveSource, /h-5 gap-\[2\.5px\]/);
  assert.match(voiceWaveSource, /w-\[2\.5px\]/);
  assert.match(voiceWaveSource, /h-6 w-\[72px\] justify-between/);
  assert.match(voiceWaveSource, /w-\[3px\]/);
  assert.match(voiceWaveSource, /w-\[3\.5px\]/);
  assert.match(voiceWaveSource, /size === "large"/);
  assert.match(voiceWaveSource, /h-7 gap-\[3px\]/);
  assert.match(voiceWaveSource, /w-\[3px\]/);
  assert.match(voiceWaveSource, /const motionClass = isRecording[\s\S]*"wave-bar"[\s\S]*"wave-processing"[\s\S]*transition-\[opacity,transform,height\]/);
});

test("record tab keeps the open record surface and guided recovery states", () => {
  const recordTabBlock = getRecordTabBlock();

  assert.match(mainViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(recordTabBlock, /<VoiceWave mode=\{waveMode\} size="large" \/>/);
  assert.match(recordTabBlock, /rounded-\[18px\] border border-white\/10 bg-white\/\[0\.03\]/);
  assert.match(recordTabBlock, /<p className="text-xs leading-5 text-slate-100">\{transcript\}<\/p>/);
  assert.match(recordTabBlock, /<p className="text-xs leading-5 text-slate-400">\{stageHint\}<\/p>/);
  assert.match(recordTabBlock, /Review the error details, then try again or open settings\./);
  assert.match(mainViewSource, /status === "idle" && !transcript && \(missingTranscriptionKey \|\| missingPromptOptimizerKey\) && \(/);
  assert.match(recordTabBlock, /Open settings to add the missing API key before you dictate\./);
});

test("widget does not expose an update-ready indicator or tooltip", () => {
  assert.doesNotMatch(widgetViewSource, /updateReady/);
  assert.doesNotMatch(widgetViewSource, /Update ready/);
  assert.doesNotMatch(widgetViewSource, /right-click to restart/);
});

test("widget keeps the compact lockup and exposes a visible settings action", () => {
  assert.doesNotMatch(widgetViewSource, />Fam</);
  assert.match(widgetViewSource, /className="widget-status/);
  assert.match(
    widgetViewSource,
    /className="widget-shell relative rounded-\[18px\] px-2 py-2 overflow-hidden"/,
  );
  assert.match(widgetViewSource, /className="flex items-center gap-2\.5 pointer-events-none select-none"/);
  assert.match(widgetViewSource, /const settingsAction = \(/);
  assert.match(widgetViewSource, /Open settings/);
  assert.match(widgetViewSource, /<FamVoiceLockup markSize=\{26\} \/>/);
  assert.match(widgetViewSource, /<FamVoiceLockup aria-hidden="true" markSize=\{26\} wordmarkClassName="opacity-0" \/>/);
  assert.match(widgetViewSource, /className="widget-status relative flex min-w-0 items-center justify-center pointer-events-none select-none"/);
  assert.match(widgetViewSource, /className="absolute inset-0 flex items-center justify-center"/);
  assert.match(widgetViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(widgetViewSource, /const showIssue = status === "error" \|\| \(status === "idle" && missingApiKey\);/);
  assert.match(widgetViewSource, /const statusLabel = status === "error" \? "Transcription error" : "Missing API key";/);
  assert.match(widgetViewSource, /No voice detected\. Try again with a clearer input\./);
  assert.match(widgetViewSource, /Check your microphone or input source, then try again\./);
  assert.match(widgetViewSource, /Add your API key in Settings to start dictating\./);
  assert.match(widgetViewSource, /<VoiceWave mode=\{/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{waveMode\} size="widget" \/>/);
  assert.doesNotMatch(widgetViewSource, /shadow-\[0_0_15px_rgba\(255,81,47,0\.4\)\]/);
  assert.doesNotMatch(widgetViewSource, /status === "transcribing" && \(/);
  assert.doesNotMatch(widgetViewSource, /animate-pulse/);
});

test("widget keeps errors inline instead of a tooltip title", () => {
  assert.doesNotMatch(widgetViewSource, /title=/);
  assert.match(widgetViewSource, /<p className="text-sm leading-5 text-slate-400">\s*\{statusCopy\}\s*<\/p>/);
  assert.match(widgetViewSource, /Open settings/);
});
