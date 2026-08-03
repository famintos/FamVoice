import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const ciWorkflowSource = readFileSync(
  new URL("../.github/workflows/ci.yml", import.meta.url),
  "utf8",
).replace(/\r\n?/g, "\n");
const releaseWorkflowSource = readFileSync(
  new URL("../.github/workflows/release.yml", import.meta.url),
  "utf8",
).replace(/\r\n?/g, "\n");
const codeqlWorkflowSource = readFileSync(
  new URL("../.github/workflows/codeql.yml", import.meta.url),
  "utf8",
).replace(/\r\n?/g, "\n");
const dependabotSource = readFileSync(
  new URL("../.github/dependabot.yml", import.meta.url),
  "utf8",
).replace(/\r\n?/g, "\n");

function actionRefs(workflowSource) {
  return [...workflowSource.matchAll(/^\s*uses:\s*([^\s#]+)(?:\s+#.*)?$/gm)]
    .map((match) => match[1]);
}

function findActionRefs(workflowSource, actionName) {
  return actionRefs(workflowSource).filter((reference) =>
    reference.startsWith(`${actionName}@`),
  );
}

function actionRevision(reference) {
  return reference.slice(reference.lastIndexOf("@") + 1);
}

test("ci workflow runs frontend tests through the project npm script", () => {
  assert.match(ciWorkflowSource, /name:\s*Run frontend tests/);
  assert.match(ciWorkflowSource, /run:\s*npm run test/);
  assert.doesNotMatch(ciWorkflowSource, /run:\s*node --test src\/\*\.test\.mjs/);
});

test("ci workflow installs a pinned cargo-audit binary and audits from src-tauri", () => {
  assert.match(ciWorkflowSource, /name:\s*Install cargo-audit/);
  const installActionRefs = findActionRefs(ciWorkflowSource, "taiki-e/install-action");
  assert.equal(installActionRefs.length, 1);
  assert.match(actionRevision(installActionRefs[0]), /^[0-9a-f]{40}$/);
  assert.match(ciWorkflowSource, /tool:\s*cargo-audit@\d+\.\d+\.\d+/);
  assert.match(ciWorkflowSource, /fallback:\s*none/);
  assert.match(ciWorkflowSource, /name:\s*Security audit/);
  assert.match(ciWorkflowSource, /working-directory:\s*src-tauri/);
  assert.match(ciWorkflowSource, /cargo audit/);
  assert.doesNotMatch(ciWorkflowSource, /cargo install cargo-audit/);
  assert.doesNotMatch(ciWorkflowSource, /cargo audit --manifest-path/);
});

test("clippy gates match the strict local command in every workflow", () => {
  for (const [name, source] of [
    ["ci", ciWorkflowSource],
    ["release", releaseWorkflowSource],
  ]) {
    const clippyCommands = [...source.matchAll(/^\s*run:\s*(cargo clippy .+)$/gm)]
      .map((match) => match[1].trim());

    assert.equal(
      clippyCommands.length,
      1,
      `${name} workflow must run clippy exactly once`,
    );
    assert.equal(
      clippyCommands[0],
      "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings",
      `${name} clippy must lint all targets and features, like the local gate`,
    );
  }
});

test("ci workflow ignores release tags on push", () => {
  assert.match(ciWorkflowSource, /push:\s*\n(?:.*\n)*?\s+tags-ignore:\s*\n\s+-\s+'v\*'/);
});

test("published workflow actions stay pinned to immutable revisions", () => {
  for (const [name, source] of [
    ["ci", ciWorkflowSource],
    ["release", releaseWorkflowSource],
    ["codeql", codeqlWorkflowSource],
  ]) {
    const refs = actionRefs(source);
    assert.ok(refs.length > 0, `${name} workflow must use at least one action`);
    for (const reference of refs) {
      assert.match(
        actionRevision(reference),
        /^[0-9a-f]{40}$/,
        `${reference} in ${name} must be pinned to a full commit SHA`,
      );
    }
  }
});

test("CodeQL init and analyze always use the same pinned revision", () => {
  const initRefs = findActionRefs(codeqlWorkflowSource, "github/codeql-action/init");
  const analyzeRefs = findActionRefs(codeqlWorkflowSource, "github/codeql-action/analyze");

  assert.equal(initRefs.length, 1);
  assert.equal(analyzeRefs.length, 1);
  assert.equal(actionRevision(initRefs[0]), actionRevision(analyzeRefs[0]));
});

test("Dependabot groups coupled CodeQL action updates", () => {
  assert.match(dependabotSource, /groups:\s*\n\s+codeql-action:/);
  assert.match(dependabotSource, /-\s+"github\/codeql-action\/\*"/);
});
