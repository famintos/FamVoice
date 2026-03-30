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

  return mainViewSource.slice(recordTabIndex, recordTabIndex + 3600);
}

test("widget container uses manual dragging instead of native drag-region", () => {
  const widgetBranchBlock = getWidgetBranchBlock();

  assert.equal(widgetViewSource.includes("data-tauri-drag-region"), false);
  assert.match(widgetViewSource, /className=\{shellClassName\}/);
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
  assert.match(voiceWaveSource, /const PROFILE_PRESETS = \{/);
  assert.match(voiceWaveSource, /size === "widget"/);
  assert.match(voiceWaveSource, /h-5 gap-\[2px\] justify-center/);
  assert.match(voiceWaveSource, /h-6 w-full justify-center gap-\[1px\] px-0/);
  assert.match(voiceWaveSource, /w-\[3px\]/);
  assert.match(voiceWaveSource, /w-\[4\.5px\]/);
  assert.match(voiceWaveSource, /w-\[3\.5px\]/);
  assert.match(voiceWaveSource, /size === "large"/);
  assert.match(voiceWaveSource, /h-12 gap-\[3px\] justify-center/);
  assert.match(voiceWaveSource, /bg-primary/);
  assert.doesNotMatch(voiceWaveSource, /bg-gradient-to-t/);
  assert.match(voiceWaveSource, /transition-\[opacity,height\]/);
  assert.match(voiceWaveSource, /\["--bar-rest-scale" as any\]/);
  assert.match(voiceWaveSource, /\["--bar-active-scale" as any\]/);
  assert.match(voiceWaveSource, /wave-bar/);
  assert.match(voiceWaveSource, /wave-processing wave-shimmer/);
});

test("record tab keeps the open record surface and guided recovery states", () => {
  const recordTabBlock = getRecordTabBlock();

  assert.match(mainViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(mainViewSource, /const showSettingsNotice = status === "idle" && !transcript && \(missingTranscriptionKey \|\| missingPromptOptimizerKey\);/);
  assert.match(mainViewSource, /const showRecordError = status === "error" && Boolean\(transcript\);/);
  assert.match(mainViewSource, /const showRecordTranscript = !showRecordError && Boolean\(transcript\);/);
  assert.match(recordTabBlock, /<VoiceWave mode=\{waveMode\} size="large" \/>/);
  assert.match(recordTabBlock, /flex flex-1 flex-col items-center justify-center rounded-\[18px\] border border-white\/10 bg-white\/\[0\.03\] px-3 pt-1 pb-3 no-drag text-center/);
  assert.match(recordTabBlock, /rounded-\[18px\] border border-white\/10 bg-white\/\[0\.03\]/);
  assert.match(recordTabBlock, /flex flex-col items-center gap-1\.5/);
  assert.match(recordTabBlock, /flex min-h-\[2\.75rem\] w-full max-w-\[16rem\] items-start justify-center/);
  assert.match(recordTabBlock, /<p className="text-\[11px\] font-medium leading-tight text-red-50">\{transcript\}<\/p>/);
  assert.match(recordTabBlock, /custom-scrollbar max-h-\[2\.75rem\] overflow-y-auto px-1/);
  assert.match(recordTabBlock, /Try again or check settings\./);
  assert.match(recordTabBlock, /Add API key in settings\./);
  assert.match(recordTabBlock, /<div className="h-\[2\.75rem\]" aria-hidden="true" \/>/);
});

test("widget does not expose an update-ready indicator or tooltip", () => {
  assert.doesNotMatch(widgetViewSource, /updateReady/);
  assert.doesNotMatch(widgetViewSource, /Update ready/);
  assert.doesNotMatch(widgetViewSource, /right-click to restart/);
});

test("widget keeps the compact lockup without a settings action", () => {
  assert.doesNotMatch(widgetViewSource, />Fam</);
  assert.match(widgetViewSource, /className="widget-status/);
  assert.match(widgetViewSource, /const shellClassName = isCompactWaveState/);
  assert.match(widgetViewSource, /widget-shell widget-shell--compact relative rounded-\[16px\] pl-1\.5 pr-0\.5 py-1\.5 overflow-hidden/);
  assert.match(widgetViewSource, /widget-shell relative rounded-\[16px\] pl-2 pr-1 py-1\.5 overflow-hidden/);
  assert.match(widgetViewSource, /const rowClassName = isCompactWaveState/);
  assert.match(widgetViewSource, /gap-1\.5/);
  assert.match(widgetViewSource, /gap-2\.5/);
  assert.doesNotMatch(widgetViewSource, /const settingsAction = \(/);
  assert.doesNotMatch(widgetViewSource, /aria-label="Settings"/);
  assert.match(widgetViewSource, /<FamVoiceLockup markSize=\{22\} \/>/);
  assert.match(widgetViewSource, /<FamVoiceLogo size=\{activeMarkSize\} className="shrink-0" \/>/);
  assert.match(widgetViewSource, /className="widget-status flex min-w-0 items-center justify-center pointer-events-none select-none"/);
  assert.match(widgetViewSource, /const renderedWaveMode = waveMode === "idle" && isFinishing \? "transcribing" : waveMode;/);
  assert.match(widgetViewSource, /const waveWrapClassName = isFinishing/);
  assert.match(widgetViewSource, /widget-wave-wrap widget-wave-wrap--finish/);
  assert.match(widgetViewSource, /const waveMode = status === "transcribing" \? "transcribing" : status === "recording" \? "recording" : "idle";/);
  assert.match(widgetViewSource, /const \[isFinishing, setIsFinishing\] = useState\(false\);/);
  assert.match(widgetViewSource, /const previousStatusRef = useRef<Status>\(status\);/);
  assert.match(widgetViewSource, /previousStatus === "recording" && \(status === "transcribing" \|\| status === "success"\)/);
  assert.match(widgetViewSource, /const showError = status === "error";/);
  assert.match(widgetViewSource, /const showIssue = showError \|\| \(status === "idle" && missingApiKey\);/);
  assert.match(widgetViewSource, /const statusLabel = showError \? "Error" : "API key missing";/);
  assert.match(widgetViewSource, /const widgetSizeAnchor = \(/);
  assert.match(widgetViewSource, /className="pointer-events-none invisible"/);
  assert.match(widgetViewSource, /\{widgetSizeAnchor\}/);
  assert.match(widgetViewSource, /No speech found\./);
  assert.match(widgetViewSource, /Try again\./);
  assert.match(widgetViewSource, /Open Settings\./);
  assert.match(widgetViewSource, /<VoiceWave mode=\{/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{renderedWaveMode\} size="widget" \/>/);
  assert.match(widgetViewSource, /flex w-full items-center pl-1\.5 pr-0\.5 py-1/);
  assert.match(widgetViewSource, /flex w-full items-center pl-1 pr-0 py-1/);
  assert.doesNotMatch(widgetViewSource, /shadow-\[0_0_15px_rgba\(255,81,47,0\.4\)\]/);
  assert.doesNotMatch(widgetViewSource, /status === "transcribing" && \(/);
  assert.doesNotMatch(widgetViewSource, /animate-pulse/);
});

test("widget keeps errors inline instead of a tooltip title", () => {
  assert.doesNotMatch(widgetViewSource, /title=/);
  assert.match(widgetViewSource, /<p className="truncate text-\[9px\] leading-tight text-slate-400">\s*\{statusCopy\}\s*<\/p>/);
  assert.doesNotMatch(widgetViewSource, /aria-label="Settings"/);
});
