import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const tauriConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const cargoToml = readFileSync(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");

test("release metadata keeps package, Tauri, and Cargo versions aligned", () => {
  const cargoVersionMatch = cargoToml.match(/^version = "([^"]+)"$/m);
  assert.ok(cargoVersionMatch, "expected Cargo.toml version entry");
  assert.equal(packageJson.version, tauriConfig.version);
  assert.equal(packageJson.version, cargoVersionMatch[1]);
});
