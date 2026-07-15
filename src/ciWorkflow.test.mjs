import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const ciWorkflowSource = readFileSync(
  new URL("../.github/workflows/ci.yml", import.meta.url),
  "utf8",
);

test("ci workflow runs frontend tests through the project npm script", () => {
  assert.match(ciWorkflowSource, /name:\s*Run frontend tests/);
  assert.match(ciWorkflowSource, /run:\s*npm run test/);
  assert.doesNotMatch(ciWorkflowSource, /run:\s*node --test src\/\*\.test\.mjs/);
});

test("ci workflow installs a pinned cargo-audit binary and audits from src-tauri", () => {
  assert.match(ciWorkflowSource, /name:\s*Install cargo-audit/);
  assert.match(
    ciWorkflowSource,
    /uses:\s*taiki-e\/install-action@43aecc8d72668fbcfe75c31400bc4f890f1c5853/,
  );
  assert.match(ciWorkflowSource, /tool:\s*cargo-audit@0\.22\.1/);
  assert.match(ciWorkflowSource, /fallback:\s*none/);
  assert.match(ciWorkflowSource, /name:\s*Security audit/);
  assert.match(ciWorkflowSource, /working-directory:\s*src-tauri/);
  assert.match(ciWorkflowSource, /cargo audit/);
  assert.doesNotMatch(ciWorkflowSource, /cargo install cargo-audit/);
  assert.doesNotMatch(ciWorkflowSource, /cargo audit --manifest-path/);
});

test("ci workflow ignores release tags on push", () => {
  assert.match(ciWorkflowSource, /push:\s*\n(?:.*\n)*?\s+tags-ignore:\s*\n\s+-\s+'v\*'/);
});
