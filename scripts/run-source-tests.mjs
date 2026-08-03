#!/usr/bin/env node
// node --test's own CLI glob-pattern resolution is unreliable on Windows with
// Node 20.19.0 (returns zero matches for quoted "*" patterns), so this script
// resolves the test file list itself with plain fs.readdirSync and hands
// node --test a literal file list instead.
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

function findTopLevel(dir, suffix) {
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(suffix))
    .map((entry) => join(dir, entry.name));
}

function findRecursive(dir, suffix) {
  return readdirSync(dir, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(suffix))
    .map((entry) => join(entry.parentPath ?? entry.path, entry.name));
}

const files = [
  ...findTopLevel("src", ".test.mjs"),
  ...findRecursive("src-tauri", ".test.mjs"),
].sort();

if (files.length === 0) {
  console.error("Could not find any *.test.mjs files under src/ or src-tauri/");
  process.exit(1);
}

const result = spawnSync(process.execPath, ["--test", ...files], {
  stdio: "inherit",
});

process.exit(result.status ?? 1);
