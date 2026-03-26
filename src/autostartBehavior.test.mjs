import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8");

test("settings view asks the backend whether autostart is available", () => {
  assert.match(settingsViewSource, /invoke<boolean>\("can_manage_autostart"\)/);
});

test("launch on startup toggle is disabled when the current executable is unsafe for autostart", () => {
  assert.match(settingsViewSource, /disabled=\{!autostartAvailable\}/);
  assert.match(
    settingsViewSource,
    /Launch on Startup is only available from the installed app\./,
  );
});
