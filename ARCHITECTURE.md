# FamVoice Architecture

FamVoice uses a standard Tauri architecture, combining a lightweight Rust backend with a React/TypeScript frontend.

## Frontend (`src/`)

- **React + TypeScript + Vite:** Handles the UI rendering and user interactions.
- **Tailwind CSS:** Used for styling the application with a modern, dark-mode aesthetic.
- **Tauri API (`@tauri-apps/api`):** Communicates with the Rust backend via IPC (Inter-Process Communication).
- **Web Audio API:** Generates simple sine/sawtooth waves for auditory feedback without requiring external asset files.

## Backend (`src-tauri/`)

- **Tauri (v2):** Manages the system tray, window lifecycle, and global shortcuts.
- **Audio (`audio.rs`):** Uses `cpal` to capture microphone input and encodes WAV in-memory without external crate dependencies.
- **Transcription (`transcription.rs`):** Posts the WAV file to the selected provider's audio transcription endpoint (OpenAI or Groq) using `reqwest`.
- **Clipboard (`clipboard.rs`):** Interacts with the system clipboard using `arboard` to safely read, store, and write text.
- **Injection (`injection.rs`):** Uses `enigo` to simulate native paste keystrokes (`Shift+Insert` on Windows, `Cmd+V` on macOS, `Ctrl+V` elsewhere), pasting the transcribed text directly into the user's active window.
- **Settings (`settings.rs`):** Atomically persists user preferences, stores provider API keys in Windows Credential Manager with a DPAPI-encrypted recovery copy, validates provider/model pairs, and owns versioned transcription-model migration.
- **History (`history.rs`):** Maintains a rolling log of recent transcripts, serialized to disk, enabling users to re-paste or review old dictations.

## IPC Commands

The frontend invokes various Rust commands registered in `lib.rs`:
- `get_settings` / `save_settings`
- `get_history` / `clear_history` / `delete_history_item` / `repaste_history_item`
- `start_recording_cmd` / `stop_recording_cmd`
- `resize_main_window` — resizes the main window (used for widget mode transitions)
- `open_settings_window` — opens the settings window positioned alongside the main window
- `close_settings_window` — closes the settings window
- `can_manage_autostart` — checks if autostart management is available (blocked in dev builds)

Event-driven architecture is used to stream statuses (`recording`, `transcribing`, `success`, `error`) and the final `transcript` back to the frontend.

## Transcription model contract

`settings.rs` is the authority for supported provider/model pairs and named defaults. OpenAI defaults to `gpt-transcribe`; Groq defaults to `whisper-large-v3-turbo`. Normalization does not rely on the order of model arrays.

Persisted settings include `transcription_model_settings_version`. Version `1` distinguishes a deliberate, current `whisper-1` selection from an unversioned legacy OpenAI value: the latter migrates once to `gpt-transcribe` and exposes a sanitized notice through `FrontendSettings.transcription_model_notice`. Valid Groq selections are not rewritten by this migration. `save_settings` validates the resulting provider/model pair before persistence.

The transcription client uploads a completed recording file. Streaming-capable OpenAI models can return file-transcription deltas, while `whisper-1` remains on the non-streaming path. Provider capabilities, request fields, pricing sources, and the pt-PT evaluation protocol are documented in [docs/transcription-models.md](docs/transcription-models.md).
