import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const mainSource = readFileSync(new URL("./main.tsx", import.meta.url), "utf8");
const cssSource = readFileSync(new URL("./App.css", import.meta.url), "utf8");
const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

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
