# Manual Update Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace startup auto-install with a manual update flow that shows a one-time notice and requires the user to apply updates from Settings.

**Architecture:** Keep updater discovery in the frontend. `MainView` performs the automatic startup check and owns the one-time in-session notice. `SettingsView` performs its own update check, shows manual update status, and applies the update with `downloadAndInstall()` followed by `relaunch()`.

**Tech Stack:** React 19, TypeScript, Tauri updater plugin, Tauri process plugin, Node built-in test runner

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/MainView.tsx` | Remove startup auto-install, add one-time update notice, remove widget restart shortcut |
| Modify | `src/SettingsView.tsx` | Check update availability, render update section, apply manual update, refresh on focus |
| Modify | `src/WidgetView.tsx` | Remove update-ready widget indicator |
| Modify | `src/widgetBehavior.test.mjs` | Lock widget behavior and update notice expectations |
| Create | `src/updateFlow.test.mjs` | Lock startup/update settings behavior expectations |

### Task 1: Add failing tests for the manual update flow

**Files:**
- Modify: `src/widgetBehavior.test.mjs`
- Create: `src/updateFlow.test.mjs`

- [ ] **Step 1: Write the failing tests**

Add tests that assert:

- `MainView` no longer calls `downloadAndInstall()` during the startup `check()` effect
- `MainView` renders a dismissible update notice that includes the available version
- dismissing the notice keeps the available update in session state but prevents the notice from reopening during the same launch
- the notice offers an action that opens Settings instead of applying the update directly
- widget mode right-click opens Settings instead of restarting an update
- `WidgetView` no longer renders an update-ready indicator
- `SettingsView` imports updater/process APIs, renders an update section, and applies `downloadAndInstall()` then `relaunch()`
- `SettingsView` refreshes update availability on mount and when focus returns
- `SettingsView` logs check failures and recovers to a neutral non-blocking state

- [ ] **Step 2: Run tests to verify they fail**

Run: `node --test src/updateFlow.test.mjs src/widgetBehavior.test.mjs`
Expected: FAIL because the current source still auto-installs updates, keeps the widget update indicator, and has no Settings update section.

### Task 2: Implement the main-window and widget behavior changes

**Files:**
- Modify: `src/MainView.tsx`
- Modify: `src/WidgetView.tsx`

- [ ] **Step 1: Write the minimal implementation**

In `src/MainView.tsx`:

- keep the startup `check()` effect
- remove `await update.downloadAndInstall()` from that effect
- track available update plus notice visibility/dismissal in component state
- render a dismissible notice that opens Settings
- remove the old "ready to restart" button
- make widget mode always route right-click to Settings
- stop passing widget update state into `WidgetView`

In `src/WidgetView.tsx`:

- remove the `updateReady` prop
- remove the green update indicator and restart-oriented tooltip copy

- [ ] **Step 2: Run tests to verify progress**

Run: `node --test src/updateFlow.test.mjs src/widgetBehavior.test.mjs`
Expected: widget and startup-update assertions move to PASS, Settings-specific assertions still fail until Task 3.

### Task 3: Implement the Settings manual update flow

**Files:**
- Modify: `src/SettingsView.tsx`

- [ ] **Step 1: Write the minimal implementation**

Add frontend-only update state in `SettingsView`:

- available update
- loading state for checking/applying
- inline update error

Add an effect that:

- calls `check()` on mount
- registers a focus-change listener
- re-runs `check()` when the Settings window regains focus

Add a dedicated update section that:

- shows current app version
- shows update availability
- renders a manual `Update` button when an update exists
- disables the button and shows `Updating...` while applying
- runs `await update.downloadAndInstall()` followed by `await relaunch()`
- logs refresh failures from `check()` and falls back to a neutral "no update available" state without blocking Settings
- shows inline errors if the apply step fails

- [ ] **Step 2: Run tests to verify they pass**

Run: `node --test src/updateFlow.test.mjs src/widgetBehavior.test.mjs`
Expected: PASS

### Task 4: Run final verification

**Files:**
- Modify: none

- [ ] **Step 1: Run the focused test suite**

Run: `node --test src/updateFlow.test.mjs src/widgetBehavior.test.mjs`
Expected: PASS

- [ ] **Step 2: Run the existing frontend source-based tests**

Run: `node --test src/widgetSizing.test.mjs src/widgetBehavior.test.mjs src/updateFlow.test.mjs`
Expected: PASS

- [ ] **Step 3: Run a production build**

Run: `npm run build`
Expected: build completes successfully without TypeScript errors
