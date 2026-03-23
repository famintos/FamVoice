# Manual Update Flow Design

## Goal

Change FamVoice's updater UX from silent download on startup to a manual install flow:

- the app still checks for updates automatically on startup
- the app shows a one-time in-session pop-up when an update is available
- the user must go to Settings and click `Update` to apply it
- after the update is downloaded and installed, the app restarts automatically

## Product Context

The current updater flow is too aggressive for this product. On startup, the app checks for updates, downloads and installs immediately, and then waits for a restart. That removes user control from a desktop tool that may be running in the background during work.

The target behavior is explicit and predictable:

- discovery remains automatic
- installation becomes manual
- the UI points the user toward Settings as the only place where the update can be applied

## Scope

- Keep automatic update checks on app startup.
- Stop automatic download and installation on startup.
- Show a single dismissible update notice per app launch when a new version is available.
- Add a Settings UI section that exposes update availability and a manual `Update` action.
- Apply the update only when the user clicks `Update`.
- Restart the app automatically after a successful install.

## Non-goals

- Adding a persistent "skip this version" preference.
- Persisting dismissed update notices across launches.
- Moving update checks from the frontend to Rust.
- Adding release notes, changelog rendering, or a multi-step installer UI.
- Introducing a separate native dialog window for updates.

## Why This Path

The current updater logic already lives in the frontend via `@tauri-apps/plugin-updater` and `@tauri-apps/plugin-process`. Keeping the flow in the frontend is the smallest safe change because it:

- preserves the current architecture
- avoids new IPC or backend state management
- matches the user's requirement that only the install action becomes manual

The user only asked to change behavior, not to redesign the updater subsystem.

## Design

### 1. Startup Check Remains Automatic

`MainView` should continue to call `check()` during startup.

The key behavior change is that a positive result no longer triggers `downloadAndInstall()` during this effect. Instead, the app stores the returned `Update` object in session state and uses it only for UI.

Expected startup outcomes:

- if no update is available, do nothing
- if an update is available, show the notice
- if the check fails, log the error and continue normally

### 2. One-Time Per Launch Notice

When startup finds an available update, the main window should show a small dismissible update notice.

The notice should:

- say that a new update is available
- include the target version
- provide an action that opens Settings
- provide a dismiss action

Dismissal behavior:

- dismissing the notice hides it for the rest of the current app launch
- closing and reopening the app resets that dismissal state
- the update remains available in Settings even after the notice is dismissed

This satisfies the requirement that the pop-up appears once per launch and does not keep returning until the next startup.

### 3. Settings Becomes the Only Update Action Surface

Settings should gain a dedicated update section.

Recommended states:

- no update available: show current app version and a neutral status
- update available: show the available version and an `Update` button
- applying update: disable the button and show progress text such as `Updating...`
- apply failure: keep the section visible and show the error inline

The Settings window is the only place where the user can start the update. The main window notice should not install the update directly.

### 4. Manual Apply Flow

Clicking `Update` in Settings should run:

1. `downloadAndInstall()`
2. `relaunch()`

This flow should happen in one click. There is no separate "restart now" step.

If installation fails:

- do not close Settings
- clear the loading state
- show the error inline
- allow the user to retry

### 5. Widget Mode Behavior

Widget mode should no longer treat update availability as a restart shortcut.

That means:

- right-click should keep opening Settings
- widget mode should not apply updates directly
- any update notice behavior remains tied to the normal main window flow

This keeps the update path explicit and avoids surprising restarts from a compact background UI.

### 6. Session State Shape

The update state should remain frontend-only and session-scoped.

Recommended state:

- `availableUpdate: Update | null`
- `isUpdateNoticeOpen: boolean`
- `hasDismissedUpdateNotice: boolean`
- `isApplyingUpdate: boolean`
- `updateError: string | null`

This state should not be persisted to the JSON settings file. Dismissal and availability are runtime concerns, not user preferences.

### 7. Sharing Update Availability With Settings

Because `MainView` and `SettingsView` are separate windows, the implementation needs a lightweight way for Settings to know whether an update is available.

Recommended approach:

- `MainView` still performs the startup check
- `SettingsView` performs its own `check()` when it opens or when it needs update state

This duplicates a lightweight network call, but it avoids adding new event plumbing or backend coordination just to share ephemeral updater state. The cost is low and the implementation stays simple.

If the updater plugin returns no update in Settings, that should be treated as "no update available" rather than an error.

## Error Handling

- Startup check failures should only be logged and must not block the app.
- Update apply failures should remain visible in Settings until the user retries or closes the window.
- The app should prevent duplicate update actions while an install is in progress.
- Settings should not close automatically unless the update succeeds and `relaunch()` takes over.

## Testing

- Add or update frontend tests so startup update checks no longer call `downloadAndInstall()` automatically.
- Add coverage for the one-time notice behavior:
  - available update opens notice
  - dismiss hides notice
  - dismissed notice does not reopen during the same launch
- Add coverage for the Settings update action:
  - available update shows the `Update` button
  - clicking `Update` calls `downloadAndInstall()` and then `relaunch()`
  - apply failures surface inline and clear loading state
- Add coverage that widget mode no longer treats pending updates as a direct restart action.

## Acceptance Criteria

- FamVoice still checks for updates automatically on startup.
- Startup no longer downloads or installs updates automatically.
- A new update triggers a dismissible notice that appears once per app launch.
- After dismissing the notice, the update remains accessible from Settings.
- Settings exposes a manual `Update` button when a version is available.
- Clicking `Update` downloads, installs, and restarts the app automatically.
- Update failures do not close Settings and are shown inline.
