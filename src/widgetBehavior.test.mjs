import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appSource = readFileSync(new URL("./App.tsx", import.meta.url), "utf8");

function getWidgetContainerBlock() {
  const widgetIdIndex = appSource.indexOf('id="widget-container"');
  assert.notEqual(widgetIdIndex, -1, "expected widget container markup in App.tsx");

  const mainStart = appSource.lastIndexOf("<main", widgetIdIndex);
  const mainEnd = appSource.indexOf("</main>", widgetIdIndex);

  assert.notEqual(mainStart, -1, "expected widget container <main> start");
  assert.notEqual(mainEnd, -1, "expected widget container </main> end");

  return appSource.slice(mainStart, mainEnd + "</main>".length);
}

test("widget container uses manual dragging instead of native drag-region", () => {
  const widgetBlock = getWidgetContainerBlock();

  assert.equal(widgetBlock.includes("data-tauri-drag-region"), false);
  assert.match(widgetBlock, /onMouseDown=\{\(e\) => \{/);
  assert.match(widgetBlock, /appWindow\.startDragging\(\)/);
});

test("widget right click opens settings from the widget container", () => {
  const widgetBlock = getWidgetContainerBlock();

  assert.match(widgetBlock, /onContextMenu=\{\(e\) => \{/);
  assert.match(widgetBlock, /void handleOpenSettings\(\)/);
});
