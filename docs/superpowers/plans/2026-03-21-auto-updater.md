# Auto-Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-update FamVoice via GitHub Releases — silent download on startup, user-triggered restart.

**Architecture:** Tauri v2 updater plugin checks `latest.json` from GitHub Releases on startup. Downloads silently in background, then emits an event to the frontend. Main view shows an amber banner, widget mode shows a green dot. GitHub Actions builds and publishes signed NSIS installers on version tags.

**Tech Stack:** tauri-plugin-updater, tauri-plugin-process, @tauri-apps/plugin-updater, @tauri-apps/plugin-process, GitHub Actions with tauri-action

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/Cargo.toml` | Add updater + process plugin deps |
| Modify | `src-tauri/tauri.conf.json` | Updater config, signing pubkey, endpoint |
| Modify | `src-tauri/src/lib.rs` | Register plugins, startup update check |
| Modify | `src/MainView.tsx` | Update-ready banner |
| Modify | `src/WidgetView.tsx` | Update-ready dot + prop |
| Modify | `src/appTypes.ts` | (none expected, events use string payloads) |
| Modify | `package.json` | Add @tauri-apps/plugin-updater + plugin-process |
| Create | `.github/workflows/release.yml` | CI build + sign + publish |

---

### Task 1: Generate Signing Keypair

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Generate the signing key**

Run:
```bash
npx tauri signer generate -w ~/.tauri/famvoice.key
```

This outputs a public key to stdout and saves the private key to `~/.tauri/famvoice.key`. Copy the public key (the long base64 string starting with `dW5...`).

- [ ] **Step 2: Add updater config to tauri.conf.json**

In `src-tauri/tauri.conf.json`, add `createUpdaterArtifacts` to `bundle` and add the `plugins.updater` section:

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true,
    "icon": [...]
  },
  "plugins": {
    "updater": {
      "pubkey": "<PASTE PUBLIC KEY HERE>",
      "endpoints": [
        "https://github.com/famintos/FamVoice/releases/latest/download/latest.json"
      ]
    }
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat: add updater signing config"
```

---

### Task 2: Add Rust Dependencies and Register Plugins

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add crate dependencies**

In `src-tauri/Cargo.toml` under `[dependencies]`, add:

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

- [ ] **Step 2: Register plugins in lib.rs**

In `src-tauri/src/lib.rs`, add both plugins to the builder chain (before `.setup()`):

```rust
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())
```

- [ ] **Step 3: Build to verify**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs
git commit -m "feat: add updater and process plugins"
```

---

### Task 3: Add Frontend Dependencies

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Install npm packages**

```bash
npm install @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

- [ ] **Step 2: Commit**

```bash
git add package.json package-lock.json
git commit -m "feat: add updater and process frontend packages"
```

---

### Task 4: Implement Startup Update Check

**Files:**
- Modify: `src/MainView.tsx`

- [ ] **Step 1: Add update check logic to MainView**

In `src/MainView.tsx`, add the update check inside a new `useEffect`. Import the updater and process APIs, and add state for the update:

```typescript
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
```

Add state:
```typescript
const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
```

Add useEffect for startup check (after the existing settings useEffect):
```typescript
useEffect(() => {
  check()
    .then(async (update) => {
      if (!update) return;
      console.log(`Update available: ${update.version}`);
      await update.downloadAndInstall();
      setPendingUpdate(update);
    })
    .catch((error) => {
      console.error("Update check failed:", error);
    });
}, []);
```

This silently downloads the update. Once downloaded, `pendingUpdate` is set, which triggers the UI notification.

- [ ] **Step 2: Add restart handler**

```typescript
const handleUpdate = async () => {
  await relaunch();
};
```

- [ ] **Step 3: Build to verify**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/MainView.tsx
git commit -m "feat: startup update check with silent download"
```

---

### Task 5: Add Update-Ready UI — Main View

**Files:**
- Modify: `src/MainView.tsx`

- [ ] **Step 1: Add update banner in the record tab**

In the record tab area, add a banner for the pending update. Place it alongside the existing API key notifications (inside the `status === "idle"` block):

```tsx
{status === "idle" && !transcript && pendingUpdate && (
  <button
    onClick={handleUpdate}
    className="mt-4 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg text-[11px] text-green-300 cursor-pointer hover:bg-green-500/20 transition-all no-drag animate-in fade-in duration-300 w-full"
  >
    v{pendingUpdate.version} ready — click to restart
  </button>
)}
```

Place this BEFORE the missing API key notifications so updates are shown first.

- [ ] **Step 2: Verify visually**

Run: `npm run tauri dev`
The banner won't appear (no update available in dev), but verify no errors in console and the existing UI still works.

- [ ] **Step 3: Commit**

```bash
git add src/MainView.tsx
git commit -m "feat: update-ready banner in main view"
```

---

### Task 6: Add Update-Ready UI — Widget Mode

**Files:**
- Modify: `src/WidgetView.tsx`
- Modify: `src/MainView.tsx`

- [ ] **Step 1: Add updateReady prop to WidgetView**

In `src/WidgetView.tsx`, add the prop to the interface:

```typescript
interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  updateReady: boolean;
  containerRef: RefObject<HTMLElement | null>;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
  onContextMenu: MouseEventHandler<HTMLElement>;
}
```

Destructure it:
```typescript
export function WidgetView({
  status,
  missingApiKey,
  updateReady,
  containerRef,
  onMouseDownCapture,
  onContextMenu,
}: WidgetViewProps) {
```

- [ ] **Step 2: Add green dot indicator**

In the status indicators area of WidgetView, add after the existing missingApiKey dot:

```tsx
{status === "idle" && updateReady && (
  <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" title="Update ready — right-click to restart" />
)}
```

- [ ] **Step 3: Pass prop from MainView**

In `src/MainView.tsx`, pass the new prop to WidgetView:

```tsx
<WidgetView
  status={status}
  missingApiKey={!!missingTranscriptionKey}
  updateReady={!!pendingUpdate}
  containerRef={widgetContainerRef}
  ...
/>
```

- [ ] **Step 4: Add restart option to widget context menu**

In `src/MainView.tsx`, update the widget's onContextMenu handler to trigger restart if an update is pending, otherwise open settings:

```tsx
onContextMenu={(e) => {
  e.preventDefault();
  if (pendingUpdate) {
    void handleUpdate();
  } else {
    void handleOpenSettings();
  }
}}
```

- [ ] **Step 5: Build to verify**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add src/WidgetView.tsx src/MainView.tsx
git commit -m "feat: update-ready indicator in widget mode"
```

---

### Task 7: Create GitHub Actions Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow file**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    permissions:
      contents: write
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: lts/*
          cache: npm

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install frontend dependencies
        run: npm ci

      - name: Build and release
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: v__VERSION__
          releaseName: 'FamVoice v__VERSION__'
          releaseBody: 'See assets to download and install.'
          releaseDraft: false
          prerelease: false
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub Actions release workflow"
```

---

### Task 8: Configure GitHub Secrets and Test Release

- [ ] **Step 1: Add signing key to GitHub Secrets**

Go to `github.com/famintos/FamVoice/settings/secrets/actions` and add:

- `TAURI_SIGNING_PRIVATE_KEY` — paste the contents of `~/.tauri/famvoice.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you chose (or empty string if none)

- [ ] **Step 2: Bump version for first release**

Update the version in all three files to `0.2.0`:
- `src-tauri/tauri.conf.json` → `"version": "0.2.0"`
- `package.json` → `"version": "0.2.0"`
- `src-tauri/Cargo.toml` → `version = "0.2.0"`

- [ ] **Step 3: Commit and tag**

```bash
git add -A
git commit -m "chore: bump version to 0.2.0"
git tag v0.2.0
git push origin master --tags
```

- [ ] **Step 4: Verify GitHub Actions**

Go to `github.com/famintos/FamVoice/actions` and verify the release workflow runs successfully. Once complete, check `github.com/famintos/FamVoice/releases` for the new release with:
- `FamVoice_0.2.0_x64-setup.nsis.zip` (NSIS installer)
- `FamVoice_0.2.0_x64-setup.nsis.zip.sig` (signature)
- `latest.json` (update manifest)
