export const APP_THEME = "dark";

/**
 * FamVoice currently ships one complete visual theme. Keep the system
 * preference in the signature so light-mode support cannot be enabled by
 * accident without changing this policy and its tests.
 *
 * @param {"light" | "dark"} systemColorScheme
 * @returns {"dark"}
 */
export function resolveAppTheme(systemColorScheme) {
  void systemColorScheme;
  return APP_THEME;
}

/**
 * @param {{ dataset: DOMStringMap; style: CSSStyleDeclaration }} root
 * @param {"light" | "dark"} systemColorScheme
 */
export function applyAppTheme(root, systemColorScheme) {
  const theme = resolveAppTheme(systemColorScheme);
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
}
