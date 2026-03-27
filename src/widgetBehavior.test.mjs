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
  const headerClassIndex = settingsViewSource.indexOf('className="-mx-4 -mt-4 mb-2 px-4 pt-4 pb-3 select-none"');
  assert.notEqual(headerClassIndex, -1, "expected settings header wrapper in SettingsView.tsx");

  const headerStart = settingsViewSource.lastIndexOf("<div", headerClassIndex);
  assert.notEqual(headerStart, -1, "expected settings header wrapper start");

  return settingsViewSource.slice(headerStart, headerClassIndex + 600);
}

function getRecordTabBlock() {
  const recordTabIndex = mainViewSource.indexOf('{activeTab === "record" ? (');
  assert.notEqual(recordTabIndex, -1, "expected record tab branch in MainView.tsx");

  return mainViewSource.slice(recordTabIndex, recordTabIndex + 1200);
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

test("widget right click opens settings from the widget container", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.match(widgetViewSource, /onContextMenu=\{onContextMenu\}/);
  assert.match(widgetBranchBlock, /onContextMenu=\{\(e\) => \{/);
  assert.match(widgetBranchBlock, /void handleOpenSettings\(\)/);
  assert.doesNotMatch(widgetBranchBlock, /void handleUpdate\(\)/);
});

test("widget drag start sets a grace period before requesting the window drag", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.match(widgetBranchBlock, /widgetDragGraceUntilRef\.current = Date\.now\(\) \+ WIDGET_DRAG_START_GRACE_MS/);
  assert.match(widgetBranchBlock, /ignoreCursorEventsRef\.current = false/);
  assert.match(widgetBranchBlock, /void appWindow\.setIgnoreCursorEvents\(false\)/);
});

test("settings window uses a native drag region on the main element", () => {
  assert.match(settingsViewSource, /<main[\s\S]*data-tauri-drag-region/);
});

test("voice wave supports explicit modes and size variants", () => {
  assert.match(voiceWaveSource, /mode\?: "idle" \| "recording" \| "transcribing"/);
  assert.doesNotMatch(voiceWaveSource, /isPlaying\?: boolean/);
  assert.match(voiceWaveSource, /mode === "transcribing"/);
  assert.match(voiceWaveSource, /size = "default"/);
  assert.match(voiceWaveSource, /size\?: "default" \| "widget" \| "large"/);
  assert.match(voiceWaveSource, /size === "widget"/);
  assert.match(voiceWaveSource, /h-5 gap-\[2\.5px\]/);
  assert.match(voiceWaveSource, /w-\[2\.5px\]/);
  assert.match(voiceWaveSource, /size === "large"/);
  assert.match(voiceWaveSource, /h-8 gap-\[3px\]/);
  assert.match(voiceWaveSource, /w-\[3px\]/);
});

test("record tab keeps waves visible outside the transcribing and result states", () => {
  const recordTabBlock = getRecordTabBlock();

  assert.match(mainViewSource, /const showStatusDot = status === "success" \|\| status === "error";/);
  assert.match(mainViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(recordTabBlock, /<VoiceWave mode=\{waveMode\} size="large" \/>/);
  assert.doesNotMatch(
    recordTabBlock,
    /showStatusDot \?\s*\(\s*<div className=\{`[\s\S]*status === "transcribing" \? "bg-yellow-500 animate-pulse shadow-\[/,
  );
});

test("widget does not expose an update-ready indicator or tooltip", () => {
  assert.doesNotMatch(widgetViewSource, /updateReady/);
  assert.doesNotMatch(widgetViewSource, /Update ready/);
  assert.doesNotMatch(widgetViewSource, /right-click to restart/);
});

test("widget keeps only the logo and slightly larger waves in a more compact layout", () => {
  assert.doesNotMatch(widgetViewSource, />Fam</);
  assert.match(widgetViewSource, /className="widget-status/);
  assert.match(
    widgetViewSource,
    /className="widget-shell relative flex items-center gap-2\.5 rounded-\[18px\] px-3 py-2 overflow-hidden"/,
  );
  assert.match(widgetViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{waveMode\} size="widget" \/>/);
  assert.doesNotMatch(widgetViewSource, /shadow-\[0_0_15px_rgba\(255,81,47,0\.4\)\]/);
  assert.doesNotMatch(widgetViewSource, /status === "transcribing" && \(/);
});

test("widget exposes full backend errors through the widget shell title", () => {
  assert.match(
    widgetViewSource,
    /<main[\s\S]*title=\{status === "error" \? errorMessage \|\| "Error" : undefined\}/,
  );
});
