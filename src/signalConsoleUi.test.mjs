import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const mainSource = readFileSync(new URL("./main.tsx", import.meta.url), "utf8");
const cssSource = readFileSync(new URL("./App.css", import.meta.url), "utf8");
const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const widgetViewSource = readFileSync(new URL("./WidgetView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

function getRecordTabBlock() {
  const recordTabIndex = mainViewSource.indexOf('{activeTab === "record" ? (');
  assert.notEqual(recordTabIndex, -1, "expected record tab branch in MainView.tsx");

  return mainViewSource.slice(recordTabIndex, recordTabIndex + 2600);
}

function getHistoryTabBlock() {
  const historyTabIndex = mainViewSource.indexOf("Utility log");
  assert.notEqual(historyTabIndex, -1, "expected history tab content in MainView.tsx");

  return mainViewSource.slice(historyTabIndex - 900, historyTabIndex + 2600);
}

test("main.tsx bundles IBM Plex fonts locally", () => {
  assert.match(mainSource, /@fontsource\/ibm-plex-sans\/400\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-sans\/500\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-sans\/600\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-mono\/400\.css/);
  assert.match(mainSource, /@fontsource\/ibm-plex-mono\/500\.css/);
});

test("App.css defines the signal-console token set", () => {
  assert.match(cssSource, /--font-sans:\s*"IBM Plex Sans"/);
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
  assert.match(widgetViewSource, /className="widget-shell/);
  assert.match(widgetViewSource, /className="widget-status/);
  assert.match(widgetViewSource, /<VoiceWave mode=\{/);
});

test("MainView uses the signal-console shell and utility log structure", () => {
  assert.match(mainViewSource, /className="signal-shell signal-shell--main/);
  assert.match(mainViewSource, /className="signal-stage/);
  assert.match(mainViewSource, /className="signal-readout/);
  assert.match(mainViewSource, /className="utility-log-row/);
  assert.match(mainViewSource, /className="status-panel status-panel--update"/);
  assert.match(mainViewSource, /className="status-panel status-panel--warning"/);
  assert.doesNotMatch(mainViewSource, /bg-\[#0f0f13\]\/85 backdrop-blur-2xl/);
  assert.doesNotMatch(mainViewSource, /group-hover:opacity-100/);
});

test("MainView adapts the fixed shell with a scrollable record stack and dense history previews", () => {
  const recordTabBlock = getRecordTabBlock();
  const historyTabBlock = getHistoryTabBlock();

  assert.match(recordTabBlock, /custom-scrollbar/);
  assert.match(recordTabBlock, /overflow-y-auto/);
  assert.match(historyTabBlock, /line-clamp-2/);
});

test("SettingsView uses the signal-console settings shell and shared control surfaces", () => {
  assert.match(settingsViewSource, /className="signal-shell signal-shell--settings/);
  assert.match(settingsViewSource, /className="control-section/);
  assert.match(settingsViewSource, /className="section-eyebrow/);
  assert.match(settingsViewSource, /className="status-panel status-panel--neutral/);
  assert.match(settingsViewSource, /className="status-panel status-panel--error/);
  assert.doesNotMatch(
    settingsViewSource,
    /settings\.api_key_present \? "text-green-400" : "text-amber-400"/,
  );
  assert.doesNotMatch(
    settingsViewSource,
    /settings\.groq_api_key_present \? "text-green-400" : "text-amber-400"/,
  );
  assert.doesNotMatch(
    settingsViewSource,
    /font-mono text-amber-200">v\{availableUpdate\.version\}/,
  );
  assert.doesNotMatch(
    settingsViewSource,
    /bg-\[#0f0f13\] text-white overflow-hidden border border-white\/10 rounded-xl/,
  );
});
