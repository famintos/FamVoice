# FamVoice Agent Guide

## Agent Delegation

Subagents may be used whenever they materially improve parallelism, review quality, or execution speed. Explicit per-turn permission is not required.

## Subagent Reasoning Effort

Use `gpt-5.4-mini` by default for subagents.

Use `xhigh` by default for subagent reasoning effort.

Only use a different subagent model or reasoning effort when the task has a clear, concrete reason to justify that override.

## Project Overview

FamVoice is a lightweight global desktop dictation application built with **Tauri v2**, **React**, and **Rust**. It records audio from a global hotkey, sends that audio to OpenAI for transcription, and pastes the resulting text back into the active application. The app also supports clipboard preservation, transcript history, prompt optimization, glossary replacements, startup launch, and a minimal widget mode.

## Main Technologies

### Frontend (`src/`)

- React 19 with Vite
- TypeScript
- Tailwind CSS 4
- Lucide React icons
- Tauri API v2 (`@tauri-apps/api`) for frontend-backend IPC
- Web Audio API for simple feedback tones

### Backend (`src-tauri/`)

- Tauri v2 (Rust)
- `cpal` and `hound` for microphone capture and WAV output
- `reqwest` for OpenAI transcription requests
- `arboard` for clipboard management
- `enigo` for simulated paste keystrokes
- Local JSON persistence for settings and history

## Architecture Highlights

The application follows a standard Tauri split:

- The Rust backend handles system-level work such as recording, transcription requests, clipboard access, key injection, startup integration, and on-disk persistence.
- The React frontend renders the UI, manages settings and history views, and listens for backend status events.
- Frontend and backend communicate through Tauri IPC commands and event streams.

Key backend areas:

- `src-tauri/src/audio.rs`: microphone capture and silence trimming
- `src-tauri/src/transcription.rs`: OpenAI transcription API integration
- `src-tauri/src/clipboard.rs`: clipboard read/write behavior
- `src-tauri/src/injection.rs`: paste keystroke simulation
- `src-tauri/src/settings.rs`: JSON settings persistence
- `src-tauri/src/history.rs`: transcript history persistence
- `src-tauri/src/lib.rs`: command registration, hotkey handling, app orchestration

## IPC Surface

The frontend uses Tauri commands such as:

- `get_settings` / `save_settings`
- `get_history` / `clear_history` / `delete_history_item` / `repaste_history_item`
- `start_recording_cmd` / `stop_recording_cmd`

The backend emits status events including `recording`, `transcribing`, `success`, `error`, and `transcript`.

## Development Commands

Prerequisites:

- Node.js 18+
- Rust stable toolchain
- Tauri v2 platform prerequisites

Common commands:

```bash
npm install
npm run tauri dev
npm run build
npm run tauri build
```

## Release Process

To publish a new release:

1. Bump the version in both `package.json` and `src-tauri/tauri.conf.json` (must match)
2. Commit the version bump
3. Create and push a git tag: `git tag v<version> && git push origin v<version>`
4. GitHub Actions (`.github/workflows/release.yml`) automatically builds, signs, and publishes the release

The workflow builds on `windows-latest`, signs with `TAURI_SIGNING_PRIVATE_KEY` (repo secret), and creates a GitHub Release with the exe, msi, signatures, and `latest.json` for the auto-updater.

**Release notes are mandatory.** Every release must include a changelog-style body describing what changed. Group changes under headings like `### Fixed`, `### Added`, `### Changed`. Write from the user's perspective — focus on what they'll notice, not internal refactors.

## Development Conventions

- Keep the frontend lightweight and focused on UI state, rendering, and event handling.
- Keep system integration and OS-facing behavior in Rust.
- Preserve the existing Tauri IPC pattern instead of introducing alternate transport layers.
- Treat settings and history as local persisted JSON data unless a change explicitly requires a different storage model.
- When documenting or changing supported models, keep the OpenAI transcription model list aligned with the user-facing README.
