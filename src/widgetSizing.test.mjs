import test from "node:test";
import assert from "node:assert/strict";

import {
  LogicalSize,
  getWidgetWindowSize,
  getWidgetInteractiveBounds,
  isPointInsideBounds,
} from "./widgetSizing.js";

test("getWidgetWindowSize matches the visible widget bounds", () => {
  assert.deepEqual(getWidgetWindowSize({ width: 160, height: 44 }).toJSON(), {
    width: 160,
    height: 44,
  });
});

test("getWidgetWindowSize rounds fractional pixels without adding padding", () => {
  const size = getWidgetWindowSize({ width: 159.2, height: 43.1 });

  assert.ok(size instanceof LogicalSize);
  assert.deepEqual(size.toJSON(), {
    width: 160,
    height: 44,
  });
});

test("getWidgetWindowSize preserves DOM measurements as logical pixels", () => {
  const size = getWidgetWindowSize({ width: 160, height: 44 });

  assert.equal(size.type, "Logical");
});

test("getWidgetInteractiveBounds converts logical DOM bounds into screen pixels", () => {
  assert.deepEqual(
    getWidgetInteractiveBounds({
      rect: { left: 10, top: 5, width: 160, height: 44 },
      windowPosition: { x: 100, y: 200 },
      scaleFactor: 1.5,
    }),
    {
      left: 115,
      top: 208,
      right: 355,
      bottom: 274,
    },
  );
});

test("isPointInsideBounds detects when the cursor is inside the widget", () => {
  const bounds = {
    left: 100,
    top: 200,
    right: 260,
    bottom: 244,
  };

  assert.equal(isPointInsideBounds({ x: 120, y: 220 }, bounds), true);
  assert.equal(isPointInsideBounds({ x: 99, y: 220 }, bounds), false);
  assert.equal(isPointInsideBounds({ x: 120, y: 245 }, bounds), false);
});
