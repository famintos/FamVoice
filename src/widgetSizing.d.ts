export interface WidgetBounds {
  width: number;
  height: number;
}

export interface WidgetRect extends WidgetBounds {
  left: number;
  top: number;
}

export interface ScreenPoint {
  x: number;
  y: number;
}

export interface ScreenBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export declare class LogicalSize {
  readonly width: number;
  readonly height: number;
  readonly type: "Logical";
  constructor(width: number, height: number);
  toJSON(): {
    width: number;
    height: number;
  };
}

export const WIDGET_CHROME_PADDING: number;
export function getWidgetWindowSize(rect: WidgetBounds): LogicalSize;
export function getWidgetWindowSizeWithChrome(rect: WidgetBounds): LogicalSize;
export function getWidgetInteractiveBounds(args: {
  rect: WidgetRect;
  windowPosition: ScreenPoint;
  scaleFactor: number;
}): ScreenBounds;
export function isPointInsideBounds(point: ScreenPoint, bounds: ScreenBounds): boolean;
