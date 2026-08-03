export type SystemColorScheme = "light" | "dark";

export const APP_THEME: "dark";

export function resolveAppTheme(systemColorScheme: SystemColorScheme): "dark";

export function applyAppTheme(
  root: Pick<HTMLElement, "dataset" | "style">,
  systemColorScheme: SystemColorScheme,
): void;
