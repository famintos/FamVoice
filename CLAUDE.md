# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
npm install              # Install frontend dependencies
npm run tauri dev        # Run app in development mode (Vite dev server + Tauri)
npm run tauri build      # Build production app (MSI installer + exe)
npm run build            # TypeScript check + Vite build (frontend only)
npm run dev              # Vite dev server only (no Tauri, port 1420)
```

Rust backend is compiled via Tauri CLI — no separate `cargo build` needed. The Rust crate is at `src-tauri/`.

Tests are minimal — `src/widgetBehavior.test.mjs` and `src/widgetSizing.test.mjs` exist as plain JS test files. There's one Rust unit test in `src-tauri/src/injection.rs`.

## Architecture

Tauri v2 app: Rust backend + React 19 frontend communicating via IPC.

**Frontend** (`src/`): React + TypeScript + Tailwind CSS 4, built with Vite 7. Three main views:
- `MainView.tsx` — recording UI, history tab, update notifications, widget mode toggle
- `SettingsView.tsx` — API keys, hotkey config, model selection, glossary management
- `WidgetView.tsx` — minimal floating overlay (128x44px)

`App.tsx` routes between MainView and SettingsView based on `?view=settings` query param. The settings view opens in a separate Tauri window.

**Backend** (`src-tauri/src/`): All system-level work lives in Rust. Key modules:
- `lib.rs` — app bootstrap, IPC command registration, hotkey handling, state management
- `audio.rs` — microphone capture via cpal, resamples to 16kHz mono, VAD silence trimming
- `transcription.rs` — OpenAI and Groq API integration (multipart form POST)
- `clipboard.rs` / `injection.rs` — save/restore clipboard, simulate Ctrl+V paste
- `settings.rs` / `history.rs` — JSON file persistence in app data dir
- `glossary.rs` — case-insensitive word/phrase replacement engine
- `input_hook.rs` — global hotkey and mouse button event handling via rdev
- `prompt_optimizer/` — optional Anthropic Claude API pass to rewrite transcripts

**IPC pattern**: Frontend calls `invoke("command_name", args)` → Rust command handler. Backend pushes status updates via `app.emit("event_name", payload)` → frontend `listen()`. Key events: `status` (idle/recording/transcribing/success/error), `transcript`, `settings-updated`, `history-updated`.

**State**: Tauri-managed state structs injected into command handlers — `AudioState`, `SettingsState`, `HistoryState`, `ClipboardState`, `HttpClientState`. API keys stored in system keyring (keyring crate), other settings in JSON.

## Recording Pipeline

Hold hotkey → cpal captures mic → resample to 16kHz + VAD trim → POST to transcription API → apply glossary replacements → optional prompt optimization → save clipboard → set clipboard to text → simulate Ctrl+V → restore clipboard.

## Release Process

To publish a new release:

1. Bump the version in both `package.json` and `src-tauri/tauri.conf.json` (must match)
2. Commit the version bump
3. Create and push a git tag: `git tag v<version> && git push origin v<version>`
4. GitHub Actions (`.github/workflows/release.yml`) automatically builds, signs, and publishes the release

The workflow builds on `windows-latest`, signs with `TAURI_SIGNING_PRIVATE_KEY` (repo secret), and creates a GitHub Release with the exe, msi, signatures, and `latest.json` for the auto-updater.

The auto-updater checks `https://github.com/famintos/FamVoice/releases/latest/download/latest.json` — this file is generated automatically by `tauri-apps/tauri-action`.

**Release notes are mandatory.** Every release must include a changelog-style body describing what changed. Use the `releaseBody` field in the workflow or edit the draft release before publishing. Group changes under headings like `### Fixed`, `### Added`, `### Changed`. Write from the user's perspective — focus on what they'll notice, not internal refactors.

## Development Conventions

- Frontend handles UI state, rendering, and event handling only. System integration stays in Rust.
- Use the existing Tauri IPC command/event pattern — don't introduce alternate transport layers.
- Settings and history are local persisted JSON. API keys go through the keyring crate.
- Transcription model lists in code should stay aligned with README.md.
- The app window is small (260x200), transparent, undecorated, always-on-top, and skips the taskbar.
- Releases are Windows-only, built via GitHub Actions on tag push (v*), signed with Tauri updater keys.
