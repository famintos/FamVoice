import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

const rootUrl = new URL("..", import.meta.url);
const libRs = readText("src-tauri/src/lib.rs");

function readText(relativePath) {
  const fileUrl = new URL(relativePath, rootUrl);
  return readFileSync(fileUrl, "utf8").replace(/\r\n/g, "\n");
}

function assertExists(relativePath) {
  const fileUrl = new URL(relativePath, rootUrl);
  assert.ok(existsSync(fileUrl), `${relativePath} should exist`);
}

function assertAllExist(relativePaths, message) {
  const missing = relativePaths.filter((relativePath) => !existsSync(new URL(relativePath, rootUrl)));

  assert.deepEqual(missing, [], message);
}

function getTrayBuilderBlock() {
  const trayBuilderIndex = libRs.indexOf("TrayIconBuilder::new()");
  assert.notEqual(trayBuilderIndex, -1, "expected tray builder construction in src-tauri/src/lib.rs");

  const buildIndex = libRs.indexOf(".build(app)?;", trayBuilderIndex);
  assert.notEqual(buildIndex, -1, "expected tray builder build call in src-tauri/src/lib.rs");

  return libRs.slice(trayBuilderIndex, buildIndex + ".build(app)?;".length);
}

test("index.html points at the branded favicon instead of vite.svg", () => {
  const indexHtml = readText("index.html");

  assert.match(indexHtml, /href=["']\/favicon\.svg["']/);
  assert.doesNotMatch(indexHtml, /vite\.svg/);
});

test("public favicon is vendored", () => {
  assertExists("public/favicon.svg");
});

test("brand icon assets are vendored in src/assets/brand", () => {
  assertAllExist(
    [
      "src/assets/brand/faminto-mark-amber.svg",
      "src/assets/brand/faminto-mark-white.svg",
      "src/assets/brand/faminto-mark-black.svg",
      "src/assets/brand/faminto-app-icon.svg",
    ],
    "expected all Faminto brand assets to be vendored under src/assets/brand",
  );
});

test("tauri bundle keeps the standard generated desktop icon set", () => {
  const tauriConfig = JSON.parse(readText("src-tauri/tauri.conf.json"));

  assert.deepEqual(tauriConfig.bundle.icon, [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico",
  ]);
});

test("tray icon assets are vendored", () => {
  assertAllExist(
    [
      "src-tauri/icons/tray-icon-dark.png",
      "src-tauri/icons/tray-icon-light.png",
    ],
    "expected explicit monochrome tray icons under src-tauri/icons",
  );
});

test("tray wiring in lib.rs uses the monochrome icon instead of the default window icon", () => {
  const trayBuilderBlock = getTrayBuilderBlock();

  assert.match(trayBuilderBlock, /\.icon\(/);
  assert.match(trayBuilderBlock, /tray-icon-(?:amber|dark|light)\.png/);
  assert.doesNotMatch(trayBuilderBlock, /app\.default_window_icon\(\)\.unwrap\(\)\.clone\(\)/);
});
