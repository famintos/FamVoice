import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const indexSource = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const mainSource = readFileSync(new URL("./main.tsx", import.meta.url), "utf8");
const cssSource = readFileSync(new URL("./App.css", import.meta.url), "utf8");
const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const widgetViewSource = readFileSync(new URL("./WidgetView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

function getWidgetShellCssBlock() {
  const widgetShellMatch = cssSource.match(/\.widget-shell\s*\{[\s\S]*?\n\}/);
  assert.ok(widgetShellMatch, "expected widget-shell block in App.css");

  return widgetShellMatch[0];
}

function getRecordTabBlock() {
  const recordTabIndex = mainViewSource.indexOf('{activeTab === "record" ? (');
  assert.notEqual(recordTabIndex, -1, "expected record tab branch in MainView.tsx");

  return mainViewSource.slice(recordTabIndex, recordTabIndex + 2600);
}

function getHistoryTabBlock() {
  const historyTabIndex = mainViewSource.indexOf('className="custom-scrollbar flex-1 overflow-y-auto px-3 pb-3"');
  assert.notEqual(historyTabIndex, -1, "expected history tab content in MainView.tsx");

  return mainViewSource.slice(historyTabIndex - 300, historyTabIndex + 3200);
}

test("app shell keeps fonts local and avoids remote font providers", () => {
  assert.match(mainSource, /@fontsource\/space-grotesk\/400\.css/);
  assert.match(mainSource, /@fontsource\/space-grotesk\/500\.css/);
  assert.match(mainSource, /@fontsource\/space-grotesk\/600\.css/);
  assert.match(mainSource, /@fontsource\/space-grotesk\/700\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-mono\/400\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-mono\/500\.css/);
  assert.doesNotMatch(indexSource, /fonts\.googleapis\.com/);
  assert.doesNotMatch(indexSource, /fonts\.gstatic\.com/);
});

test("App.css defines the refreshed shell token set", () => {
  assert.match(cssSource, /--font-sans:\s*"Space Grotesk"/);
  assert.match(cssSource, /--font-mono:\s*"IBM Plex Mono"/);
  assert.match(cssSource, /--color-primary:\s*#/);
  assert.match(cssSource, /--color-surface:\s*#/);
  assert.match(cssSource, /--color-success:\s*#/);
  assert.match(cssSource, /--color-danger:\s*#/);
  assert.match(cssSource, /\.signal-shell/);
  assert.match(cssSource, /\.signal-stage/);
  assert.match(cssSource, /\.signal-readout/);
  assert.match(cssSource, /\.status-panel/);
  assert.match(cssSource, /\.utility-log-row/);
  assert.match(cssSource, /\.widget-shell/);
  assert.match(cssSource, /\.wave-processing/);
  assert.match(cssSource, /@media \(prefers-reduced-motion: reduce\)/);
});

test("WidgetView uses the compact widget shell and status surface", () => {
  assert.match(widgetViewSource, /className=\{shellClassName\}/);
  assert.match(widgetViewSource, /className="widget-status/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{/);
});

test("widget shell keeps the compact surface without an exterior shadow", () => {
  const widgetShellBlock = getWidgetShellCssBlock();

  assert.doesNotMatch(widgetShellBlock, /box-shadow:/);
});

test("MainView uses the refreshed shell with an inline update notice", () => {
  assert.match(mainViewSource, /className="signal-shell relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-\[16px\] bg-\[#161B26\]"/);
  assert.match(mainViewSource, /Update available/);
  assert.match(mainViewSource, /pendingUpdate\.version/);
  assert.match(mainViewSource, /dismissUpdateNotice/);
  assert.match(mainViewSource, /const hasDismissedUpdateNoticeRef = useRef\(false\);/);
  assert.match(mainViewSource, /if \(!hasDismissedUpdateNoticeRef\.current\) \{\s*setIsUpdateNoticeOpen\(true\);/);
});

test("MainView keeps the open record surface and guided history list", () => {
  const recordTabBlock = getRecordTabBlock();
  const historyTabBlock = getHistoryTabBlock();

  assert.match(recordTabBlock, /<VoiceWave mode=\{waveMode\} size="large" \/>/);
  assert.match(recordTabBlock, /rounded-\[18px\] border border-white\/10 bg-white\/\[0\.03\]/);
  assert.match(recordTabBlock, /flex flex-col items-center gap-1\.5/);
  assert.match(recordTabBlock, /text-\[11px\] leading-tight text-slate-400/);
  assert.match(recordTabBlock, /Try again or check settings\./);
  assert.match(mainViewSource, /Add API key in settings\./);
  assert.match(historyTabBlock, /custom-scrollbar/);
  assert.match(historyTabBlock, /copyToClipboard/);
  assert.match(historyTabBlock, /repasteHistory/);
  assert.match(mainViewSource, /first history entry/);
  assert.match(historyTabBlock, /aria-label="Copy transcript"/);
  assert.doesNotMatch(historyTabBlock, /group-hover:opacity-100/);
});

test("SettingsView uses the refreshed settings shell and inline update states", () => {
  assert.match(settingsViewSource, /className="signal-shell signal-shell--settings/);
  assert.match(settingsViewSource, /className="control-section/);
  assert.match(settingsViewSource, /className="section-eyebrow/);
  assert.match(settingsViewSource, /import \{ Select \} from "\.\/components\/Select";/);
  assert.match(settingsViewSource, /<Select/);
  assert.match(settingsViewSource, /Checking for updates\.\.\./);
  assert.match(settingsViewSource, /Could not check for updates\./);
  assert.match(settingsViewSource, /Update installation failed\./);
  assert.match(settingsViewSource, /className="mt-3 text-sm font-medium text-red-400"/);
  assert.doesNotMatch(settingsViewSource, /status-panel status-panel--neutral/);
});
