import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const releaseWorkflowSource = readFileSync(
  new URL("../.github/workflows/release.yml", import.meta.url),
  "utf8",
);

test("release workflow publishes releases instead of leaving them as drafts", () => {
  assert.match(releaseWorkflowSource, /releaseDraft:\s*false/);
});

test("release workflow loads changelog-style notes from the versioned release notes file", () => {
  assert.match(releaseWorkflowSource, /name:\s*Load release notes/);
  assert.match(releaseWorkflowSource, /id:\s*release_notes/);
  assert.match(releaseWorkflowSource, /docs\/releases\/v\$version\.md/);
  assert.match(releaseWorkflowSource, /releaseBody:\s*\$\{\{\s*steps\.release_notes\.outputs\.body\s*\}\}/);
});
