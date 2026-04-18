import type { KeyboardEvent } from "react";

export function buildHotkeyString(e: KeyboardEvent<HTMLInputElement>): string | null {
  const key = e.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  const keyMap: Record<string, string> = {
    " ": "Space",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Enter: "Enter",
    Escape: "Escape",
    Backspace: "Backspace",
    Tab: "Tab",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
  };

  const mainKey = keyMap[key] ?? (key.length === 1 ? key.toUpperCase() : key);
  parts.push(mainKey);

  return parts.join("+");
}

export function formatHotkey(hotkey: string): string {
  if (hotkey === "Mouse3") return "Mouse 3 (Middle)";
  if (hotkey === "Mouse4") return "Mouse 4 (Back)";
  if (hotkey === "Mouse5") return "Mouse 5 (Forward)";

  return hotkey
    .replace("CommandOrControl", "Ctrl")
    .replace(/\+/g, " + ");
}
