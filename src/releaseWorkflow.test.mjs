import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflowSource = readFileSync(
  new URL("../.github/workflows/release.yml", import.meta.url),
  "utf8",
).replace(/\r\n?/g, "\n");

test("release workflow publishes releases instead of leaving them as drafts", () => {
  assert.match(workflowSource, /releaseDraft:\s*false/);
});

test("release workflow loads changelog-style notes from the versioned release notes file", () => {
  assert.match(workflowSource, /name:\s*Load release notes/);
  assert.match(workflowSource, /id:\s*release_notes/);
  assert.match(workflowSource, /docs\/releases\/v\$version\.md/);
  assert.match(workflowSource, /releaseBody:\s*\$\{\{\s*steps\.release_notes\.outputs\.body\s*\}\}/);
});

test("release workflow validates tag and metadata versions before publishing", () => {
  assert.match(workflowSource, /name:\s*Validate release versions/);
  assert.match(workflowSource, /\$packageVersion =/);
  assert.match(workflowSource, /\$tauriVersion =/);
  assert.match(workflowSource, /\$cargoVersion =/);
  assert.match(workflowSource, /\$tagVersion =/);
});

test("release workflow reruns the quality gates before publishing", () => {
  assert.match(workflowSource, /name:\s*Run frontend tests/);
  assert.match(workflowSource, /name:\s*Lint frontend/);
  assert.match(workflowSource, /name:\s*Run Rust tests/);
  assert.match(workflowSource, /name:\s*Clippy lint/);
  assert.match(workflowSource, /name:\s*Security audit/);
});

test("release workflow avoids redundant checks and downloads cargo-audit", () => {
  assert.doesNotMatch(workflowSource, /name:\s*Check Rust backend/);
  const installAction = workflowSource.match(
    /uses:\s*taiki-e\/install-action@([0-9a-f]+)(?:\s+#.*)?/,
  );
  assert.ok(installAction, "release workflow must use the binary install action");
  assert.match(installAction[1], /^[0-9a-f]{40}$/);
  assert.match(workflowSource, /tool:\s*cargo-audit@\d+\.\d+\.\d+/);
  assert.match(workflowSource, /fallback:\s*none/);
  assert.doesNotMatch(workflowSource, /cargo install cargo-audit/);
});

test("release workflow validates published updater metadata version and windows targets", () => {
  assert.match(workflowSource, /Invoke-WebRequest -Uri \$endpoint/);
  assert.match(workflowSource, /\$latestJson =/);
  assert.match(workflowSource, /\$latestJson\.version -ne \$version/);
  assert.match(workflowSource, /windows-x86_64/);
  assert.match(workflowSource, /windows-x86_64-msi/);
  assert.match(workflowSource, /windows-x86_64-nsis/);
});
