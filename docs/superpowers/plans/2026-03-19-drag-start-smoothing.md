# Drag Start Smoothing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make widget-mode and settings-window dragging start more reliably without changing their user-facing interactions.

**Architecture:** Keep the existing `App.tsx` structure and tighten the drag-start paths in place. The widget path gets a short grace period that suppresses cursor-ignore toggles during drag startup, and the settings header moves to explicit manual dragging for a more predictable start.

**Tech Stack:** React 19, TypeScript, Tauri window APIs, Node test runner

---

### Task 1: Lock the desired drag behavior with tests

**Files:**
- Modify: `src/widgetBehavior.test.mjs`
- Test: `src/widgetBehavior.test.mjs`

- [ ] **Step 1: Write the failing test**

Add assertions that:
- the widget drag starts from a capture-phase mouse handler
- the settings header uses manual drag start instead of `data-tauri-drag-region`

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test src/widgetBehavior.test.mjs`
Expected: FAIL because `App.tsx` still uses the old drag wiring.

- [ ] **Step 3: Write minimal implementation**

Update `src/App.tsx` so the widget and settings header match the tested drag behavior.

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test src/widgetBehavior.test.mjs`
Expected: PASS

### Task 2: Smooth widget drag startup

**Files:**
- Modify: `src/App.tsx`
- Test: `src/widgetBehavior.test.mjs`

- [ ] **Step 1: Write the failing test**

Add coverage for the widget drag startup path to ensure it sets drag state before calling `appWindow.startDragging()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test src/widgetBehavior.test.mjs`
Expected: FAIL because no drag-start grace state exists yet.

- [ ] **Step 3: Write minimal implementation**

Add a short grace period ref and have the widget interactivity polling bypass `setIgnoreCursorEvents(true)` while that grace period is active.

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test src/widgetBehavior.test.mjs`
Expected: PASS

### Task 3: Verify integration

**Files:**
- Modify: `src/App.tsx`
- Test: `src/widgetBehavior.test.mjs`

- [ ] **Step 1: Run the focused frontend tests**

Run: `node --test src/widgetBehavior.test.mjs src/widgetSizing.test.mjs`
Expected: PASS

- [ ] **Step 2: Run the production build**

Run: `npm run build`
Expected: PASS
