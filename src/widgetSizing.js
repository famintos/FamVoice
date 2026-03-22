import { LogicalSize } from "@tauri-apps/api/dpi";

// Extra transparent space around the widget so outline effects have room to render
export const WIDGET_CHROME_PADDING = 10;

export function getWidgetWindowSize(rect) {
  return new LogicalSize(
    Math.max(1, Math.ceil(rect.width)),
    Math.max(1, Math.ceil(rect.height)),
  );
}

export function getWidgetWindowSizeWithChrome(rect) {
  return new LogicalSize(
    Math.max(1, Math.ceil(rect.width) + WIDGET_CHROME_PADDING * 2),
    Math.max(1, Math.ceil(rect.height) + WIDGET_CHROME_PADDING * 2),
  );
}

export function getWidgetInteractiveBounds({ rect, windowPosition, scaleFactor }) {
  const left = Math.round(windowPosition.x + rect.left * scaleFactor);
  const top = Math.round(windowPosition.y + rect.top * scaleFactor);
  const right = Math.round(left + rect.width * scaleFactor);
  const bottom = Math.round(top + rect.height * scaleFactor);

  return { left, top, right, bottom };
}

export function isPointInsideBounds(point, bounds) {
  return (
    point.x >= bounds.left &&
    point.x <= bounds.right &&
    point.y >= bounds.top &&
    point.y <= bounds.bottom
  );
}
