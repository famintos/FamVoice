# FamVoice Project Overview

FamVoice is a lightweight global desktop application built with **Tauri v2**, **React**, and **Rust**. Its primary purpose is to provide seamless audio recording and fast transcription using OpenAI's APIs, followed by automatic clipboard replacement and injection of the transcribed text directly into the user's active window.

## Main Technologies

### Frontend (`src/`)
- **Framework:** React 19 with Vite
- **Language:** TypeScript
- **Styling:** Tailwind CSS 4
- **Icons:** Lucide React
- **Integration:** Tauri API v2 (`@tauri-apps/api`) for communicating with the Rust backend.
- **Audio Feedback:** Utilizes the Web Audio API to generate basic auditory feedback (sine/sawtooth waves).

### Backend (`src-tauri/`)
- **Framework:** Tauri v2 (Rust)
- **Audio Capture:** `cpal` to capture microphone input into a WAV file (via `hound`).
- **Transcription API:** `reqwest` for making API calls to OpenAI's audio transcription endpoint.
- **Clipboard Management:** `arboard` for safely reading, storing, and writing text to the system clipboard.
- **Keystroke Injection:** `enigo` to simulate keystrokes (e.g., `Ctrl+V` or `Cmd+V`) to paste transcribed text.
- **System Integration:** Global shortcut support and autostart capabilities.

## Architecture Highlights
The application uses a standard Tauri architecture. The lightweight Rust backend handles system-level tasks (audio recording, clipboard, simulating keystrokes, maintaining a rolling history on disk, and managing JSON preferences), while the React frontend handles UI rendering. They communicate via IPC commands (e.g., `get_settings`, `save_settings`, `get_history`, `start_recording_cmd`) and event streams for status updates (`recording`, `transcribing`, `success`, `error`, `transcript`).

## Building and Running

### Prerequisites
- Node.js and npm
- Rust toolchain (cargo, rustc)

### Commands

**Install Dependencies:**
```bash
npm install
```

**Run in Development Mode:**
```bash
npm run tauri dev
```

**Build for Production:**
```bash
npm run tauri build
```

## Development Conventions
- **UI Architecture:** React components styled with Tailwind CSS, supporting a modern, dark-mode aesthetic.
- **Backend Architecture:** Rust backend divided into modular files (e.g., `audio.rs`, `transcription.rs`, `clipboard.rs`, `injection.rs`, `settings.rs`, `history.rs`) managed via `lib.rs` and `main.rs`.
- **State & Data:** Settings and transcript history are serialized locally to disk as JSON.
- **Inter-Process Communication (IPC):** Use Tauri's command and event systems for frontend-backend interaction.

## Release Process

1. Bump the version in both `package.json` and `src-tauri/tauri.conf.json` (must match)
2. Commit the version bump
3. Create and push a git tag: `git tag v<version> && git push origin v<version>`
4. GitHub Actions (`.github/workflows/release.yml`) automatically builds, signs, and publishes the release

**Release notes are mandatory.** Every release must include a changelog-style body describing what changed. Group changes under headings like `### Fixed`, `### Added`, `### Changed`. Write from the user's perspective — focus on what they'll notice, not internal refactors.
