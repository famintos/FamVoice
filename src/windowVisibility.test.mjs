import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const libSource = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const windowSource = readFileSync(new URL("../src-tauri/src/window.rs", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

test("closing the main surface hides it instead of destroying it", () => {
  assert.match(mainViewSource, /onClick=\{\(\) => appWindow\.hide\(\)\}/);
  assert.doesNotMatch(mainViewSource, /onClick=\{\(\) => appWindow\.close\(\)\}/);
  assert.match(libSource, /tauri::WindowEvent::CloseRequested \{ api, \.\. \}/);
  assert.match(libSource, /api\.prevent_close\(\);\s*let _ = window\.hide\(\);/);
});

test("recording restores the widget without requesting focus", () => {
  const startIndex = libSource.indexOf("async fn start_recording_cmd");
  const startBlock = libSource.slice(startIndex, startIndex + 900);

  assert.match(startBlock, /ensure_main_window_visible\(&app, false\)/);
  assert.match(windowSource, /SW_SHOWNOACTIVATE/);
});

test("tray recovery recreates and focuses the main window", () => {
  assert.match(windowSource, /None => build_main_window\(app, widget_mode\)\?/);
  assert.match(libSource, /ensure_main_window_visible\(app, true\)/);
});

test("restored windows are clamped to a visible monitor work area", () => {
  assert.match(windowSource, /clamp_position_to_work_area/);
  assert.match(windowSource, /monitor\.work_area\(\)/);
  assert.match(windowSource, /window\s*\.set_position\(safe_position\)/);
});
