import test from "node:test";
import assert from "node:assert/strict";
import { applyAppTheme, resolveAppTheme } from "./theme.js";

function relativeLuminance(hexColor) {
  const channels = hexColor.match(/../g).map((channel) => parseInt(channel, 16) / 255);
  const linear = channels.map((channel) => (
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  ));
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrastRatio(foreground, background) {
  const lighter = Math.max(relativeLuminance(foreground), relativeLuminance(background));
  const darker = Math.min(relativeLuminance(foreground), relativeLuminance(background));
  return (lighter + 0.05) / (darker + 0.05);
}

test("FamVoice keeps the effective dark theme for light and dark system preferences", () => {
  for (const systemColorScheme of ["light", "dark"]) {
    assert.equal(resolveAppTheme(systemColorScheme), "dark");

    const root = { dataset: {}, style: {} };
    applyAppTheme(root, systemColorScheme);
    assert.equal(root.dataset.theme, "dark");
    assert.equal(root.style.colorScheme, "dark");
  }
});

test("small secondary copy uses a dark-theme color that clears WCAG AA", () => {
  assert.ok(contrastRatio("94a3b8", "0f0f0f") >= 4.5);
});
