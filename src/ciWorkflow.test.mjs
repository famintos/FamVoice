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

test("ci workflow ignores release tags on push", () => {
  assert.match(ciWorkflowSource, /push:\s*\n(?:.*\n)*?\s+tags-ignore:\s*\n\s+-\s+'v\*'/);
});
