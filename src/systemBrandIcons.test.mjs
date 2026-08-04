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
  const trayBuilderIndex = libRs.indexOf("TrayIconBuilder::with_id(tray::TRAY_ID)");
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
      "src/assets/brand/famvoice-mark-amber.svg",
      "src/assets/brand/famvoice-mark-compact-amber.svg",
      "src/assets/brand/famvoice-mark-white.svg",
      "src/assets/brand/famvoice-mark-black.svg",
      "src/assets/brand/famvoice-app-icon.svg",
      "src/assets/brand/faminto-mark-amber.svg",
      "src/assets/brand/faminto-mark-white.svg",
      "src/assets/brand/faminto-mark-black.svg",
      "src/assets/brand/faminto-app-icon.svg",
    ],
    "expected all FamVoice and Faminto brand assets to be vendored under src/assets/brand",
  );
});

test("the product mark is FamVoice, not the umbrella Faminto mark", () => {
  const logo = readText("src/FamVoiceLogo.tsx");
  const favicon = readText("public/favicon.svg");

  assert.match(logo, /famvoice-mark-amber\.svg/);
  assert.doesNotMatch(logo, /faminto-mark/);
  assert.match(favicon, /FamVoice Mark/);
  assert.doesNotMatch(favicon, /A 38 38 0 1 0/);
});

test("small renders switch to the compact optical size", () => {
  const logo = readText("src/FamVoiceLogo.tsx");
  const favicon = readText("public/favicon.svg");
  const compact = readText("src/assets/brand/famvoice-mark-compact-amber.svg");
  const regular = readText("src/assets/brand/famvoice-mark-amber.svg");

  assert.match(logo, /COMPACT_MARK_MAX_SIZE = 32/);
  assert.match(logo, /size <= COMPACT_MARK_MAX_SIZE/);

  // The tray and the browser tab both render at 16px, so both take the heavier stroke.
  assert.match(compact, /stroke-width="15"/);
  assert.match(favicon, /stroke-width="15"/);
  assert.match(regular, /stroke-width="11"/);
});

test("the tray keeps the mark in brand color at rest", () => {
  const idleFrame = readText("src/assets/brand/famvoice-mark-compact-amber.svg");

  assert.match(idleFrame, /#D17A28/);
  assert.doesNotMatch(
    idleFrame,
    /#6B727C/,
    "idle must not fall back to neutral grey: the tray is the product's only permanent presence",
  );
});

test("the off-brand gradient logo is gone", () => {
  assert.equal(
    existsSync(new URL("public/famvoice-logo.svg", rootUrl)),
    false,
    "public/famvoice-logo.svg violates BRAND.md §11 (no gradients on the symbol) and must stay deleted",
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

test("every tray pipeline frame is vendored", () => {
  assertAllExist(
    [
      "src-tauri/icons/tray-idle.png",
      "src-tauri/icons/tray-rec-0.png",
      "src-tauri/icons/tray-rec-1.png",
      "src-tauri/icons/tray-rec-2.png",
      "src-tauri/icons/tray-rec-3.png",
      "src-tauri/icons/tray-tr-0.png",
      "src-tauri/icons/tray-tr-1.png",
      "src-tauri/icons/tray-tr-2.png",
      "src-tauri/icons/tray-success.png",
    ],
    "expected one vendored tray frame per pipeline stage under src-tauri/icons",
  );
});

test("tray wiring in lib.rs starts on the idle frame, not the default window icon", () => {
  const trayBuilderBlock = getTrayBuilderBlock();

  assert.match(trayBuilderBlock, /\.icon\(tray::idle_image\(\)\)/);
  assert.doesNotMatch(trayBuilderBlock, /app\.default_window_icon\(\)\.unwrap\(\)\.clone\(\)/);
  assert.match(libRs, /tray::wire\(app\.handle\(\)\);/);
});

test("editing a tray frame actually rebuilds the binary", () => {
  const buildRs = readText("src-tauri/build.rs");

  // `include_image!` decodes these PNGs inside a proc macro, so rustc never records
  // them as source dependencies. Without this declaration, changing a frame leaves the
  // old pixels baked into the binary and no amount of rebuilding or restarting helps.
  assert.match(buildRs, /rerun-if-changed/);
  assert.match(buildRs, /starts_with\("tray-"\)/);
  assert.match(buildRs, /ends_with\("\.png"\)/);
});

test("the tray state machine subscribes to the pipeline and throttles microphone level", () => {
  const trayRs = readText("src-tauri/src/tray.rs");

  assert.match(trayRs, /app\.listen\("status"/);
  assert.match(trayRs, /app\.listen\("mic-level"/);
  assert.match(trayRs, /LEVEL_THROTTLE/);

  for (const status of ["recording", "transcribing", "success"]) {
    assert.match(trayRs, new RegExp(`"${status}" =>`), `expected the tray to handle status ${status}`);
  }
});
