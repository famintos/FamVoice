import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appConstantsSource = readFileSync(new URL("./appConstants.ts", import.meta.url), "utf8");
const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8");

test("prompt optimizer model options only expose GPT-5.4 Mini", () => {
  assert.match(appConstantsSource, /value: "gpt-5\.4-mini"/);
  assert.doesNotMatch(appConstantsSource, /value: "gpt-5\.4-nano"/);
});

test("settings view still normalizes unsupported prompt optimizer models", () => {
  assert.match(settingsViewSource, /function normalizePromptOptimizerModel\(model: string\): string/);
  assert.match(settingsViewSource, /PROMPT_OPTIMIZER_MODELS\[0\]\.value/);
});
