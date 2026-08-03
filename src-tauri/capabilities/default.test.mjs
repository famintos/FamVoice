import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const capability = JSON.parse(
  readFileSync(new URL("./default.json", import.meta.url), "utf8"),
);
const settingsCapability = JSON.parse(
  readFileSync(new URL("./settings.json", import.meta.url), "utf8"),
);

test("default capability allows manual window dragging", () => {
  assert.ok(
    capability.permissions.includes("core:window:allow-start-dragging"),
    "expected core:window:allow-start-dragging permission",
  );
});

test("default capability only exposes updater checks", () => {
  assert.ok(
    capability.permissions.includes("updater:allow-check"),
    "expected updater:allow-check permission",
  );
  assert.ok(!capability.permissions.some((permission) =>
    permission.startsWith("process:") || permission.startsWith("autostart:")
  ));
});

test("settings capability grants only used updater and process commands", () => {
  assert.ok(settingsCapability.permissions.includes("updater:allow-check"));
  assert.ok(settingsCapability.permissions.includes("updater:allow-download-and-install"));
  assert.ok(settingsCapability.permissions.includes("process:allow-restart"));
  assert.ok(!settingsCapability.permissions.includes("updater:default"));
  assert.ok(!settingsCapability.permissions.includes("process:default"));
});
