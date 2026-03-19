import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const capability = JSON.parse(
  readFileSync(new URL("./default.json", import.meta.url), "utf8"),
);

test("default capability allows manual window dragging", () => {
  assert.ok(
    capability.permissions.includes("core:window:allow-start-dragging"),
    "expected core:window:allow-start-dragging permission",
  );
});
