import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appSource = readFileSync(new URL("./App.tsx", import.meta.url), "utf8");

function getWidgetContainerBlock() {
  const widgetIdIndex = appSource.indexOf('id="widget-container"');
  assert.notEqual(widgetIdIndex, -1, "expected widget container markup in App.tsx");

  const mainStart = appSource.lastIndexOf("<main", widgetIdIndex);
  const mainEnd = appSource.indexOf("</main>", widgetIdIndex);

  assert.notEqual(mainStart, -1, "expected widget container <main> start");
  assert.notEqual(mainEnd, -1, "expected widget container </main> end");

  return appSource.slice(mainStart, mainEnd + "</main>".length);
}

function getSettingsHeaderBlock() {
  const headerClassIndex = appSource.indexOf('className="-mx-4 -mt-4 mb-2 px-4 pt-4 pb-3 select-none"');
  assert.notEqual(headerClassIndex, -1, "expected settings header wrapper in App.tsx");

  const headerStart = appSource.lastIndexOf("<div", headerClassIndex);
  assert.notEqual(headerStart, -1, "expected settings header wrapper start");

  return appSource.slice(headerStart, headerClassIndex + 600);
}

function getVoiceWaveBlock() {
  const functionStart = appSource.indexOf("function VoiceWave");
  assert.notEqual(functionStart, -1, "expected VoiceWave component in App.tsx");

  const functionEnd = appSource.indexOf("const DEFAULT_HOTKEY", functionStart);
  assert.notEqual(functionEnd, -1, "expected VoiceWave component end marker in App.tsx");

  return appSource.slice(functionStart, functionEnd);
}

function getRecordTabBlock() {
  const recordTabIndex = appSource.indexOf('{activeTab === "record" ? (');
  assert.notEqual(recordTabIndex, -1, "expected record tab branch in App.tsx");

  return appSource.slice(recordTabIndex, recordTabIndex + 1200);
}

test("widget container uses manual dragging instead of native drag-region", () => {
  const widgetBlock = getWidgetContainerBlock();

  assert.equal(widgetBlock.includes("data-tauri-drag-region"), false);
  assert.match(widgetBlock, /onMouseDownCapture=\{\(e\) => \{/);
  assert.match(widgetBlock, /appWindow\.startDragging\(\)/);
});

test("widget right click opens settings from the widget container", () => {
  const widgetBlock = getWidgetContainerBlock();

  assert.match(widgetBlock, /onContextMenu=\{\(e\) => \{/);
  assert.match(widgetBlock, /void handleOpenSettings\(\)/);
});

test("widget drag start sets a grace period before requesting the window drag", () => {
  const widgetBlock = getWidgetContainerBlock();

  assert.match(widgetBlock, /widgetDragGraceUntilRef\.current = Date\.now\(\) \+ WIDGET_DRAG_START_GRACE_MS/);
  assert.match(widgetBlock, /void appWindow\.setIgnoreCursorEvents\(false\)/);
});

test("settings header uses manual drag start instead of a native drag region", () => {
  const settingsHeaderBlock = getSettingsHeaderBlock();

  assert.equal(settingsHeaderBlock.includes("data-tauri-drag-region"), false);
  assert.match(settingsHeaderBlock, /onMouseDownCapture=\{\(e\) => \{/);
  assert.match(settingsHeaderBlock, /appWindow\.startDragging\(\)/);
});

test("voice wave supports a large size variant for the main dictation view", () => {
  const voiceWaveBlock = getVoiceWaveBlock();

  assert.match(voiceWaveBlock, /size = "default"/);
  assert.match(voiceWaveBlock, /size === "large"/);
  assert.match(voiceWaveBlock, /h-8/);
  assert.match(voiceWaveBlock, /w-\[4px\]/);
});

test("record tab keeps waves visible outside the transcribing and result states", () => {
  const recordTabBlock = getRecordTabBlock();

  assert.match(appSource, /const showStatusDot = status === "transcribing" \|\| status === "success" \|\| status === "error";/);
  assert.match(recordTabBlock, /<VoiceWave isPlaying=\{status === "recording"\} size="large" \/>/);
});
