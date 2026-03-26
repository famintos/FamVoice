import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflowSource = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");

test("release workflow publishes releases instead of leaving them as drafts", () => {
  assert.match(workflowSource, /releaseDraft:\s*false/);
});

test("release workflow loads changelog-style notes from the versioned release notes file", () => {
  assert.match(workflowSource, /name:\s*Load release notes/);
  assert.match(workflowSource, /id:\s*release_notes/);
  assert.match(workflowSource, /docs\/releases\/v\$version\.md/);
  assert.match(workflowSource, /releaseBody:\s*\$\{\{\s*steps\.release_notes\.outputs\.body\s*\}\}/);
});

test("release workflow validates published updater metadata version and windows targets", () => {
  assert.match(workflowSource, /Invoke-WebRequest -Uri \$endpoint/);
  assert.match(workflowSource, /\$latestJson =/);
  assert.match(workflowSource, /\$latestJson\.version -ne \$version/);
  assert.match(workflowSource, /windows-x86_64/);
  assert.match(workflowSource, /windows-x86_64-msi/);
  assert.match(workflowSource, /windows-x86_64-nsis/);
});
