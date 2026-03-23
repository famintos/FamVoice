# FamVoice Architecture

FamVoice uses a standard Tauri architecture, combining a lightweight Rust backend with a React/TypeScript frontend.

## Frontend (`src/`)
- **React + TypeScript + Vite:** Handles the UI rendering and user interactions.
- **Tailwind CSS:** Used for styling the application with a modern, dark-mode aesthetic.
- **Tauri API (`@tauri-apps/api`):** Communicates with the Rust backend via IPC (Inter-Process Communication).
- **Web Audio API:** Generates simple sine/sawtooth waves for auditory feedback without requiring external asset files.

## Backend (`src-tauri/`)
- **Tauri (v2):** Manages the system tray, window lifecycle, and global shortcuts.
- **Audio (`audio.rs`):** Uses `cpal` to capture microphone input directly into a WAV file via `hound`.
- **Transcription (`transcription.rs`):** Posts the WAV file to the selected provider's audio transcription endpoint (OpenAI or Groq) using `reqwest`.
- **Clipboard (`clipboard.rs`):** Interacts with the system clipboard using `arboard` to safely read, store, and write text.
- **Injection (`injection.rs`):** Uses `enigo` to simulate the `Ctrl+V` (or `Cmd+V`) keystrokes, pasting the transcribed text directly into the user's active window.
- **Settings (`settings.rs`):** Manages local JSON persistence for user preferences and stores provider API keys in the OS credential store / keyring.
- **History (`history.rs`):** Maintains a rolling log of recent transcripts, serialized to disk, enabling users to re-paste or review old dictations.

## IPC Commands
The frontend invokes various Rust commands registered in `lib.rs`:
- `get_settings` / `save_settings`
- `get_history` / `clear_history` / `delete_history_item` / `repaste_history_item`
- `start_recording_cmd` / `stop_recording_cmd`

Event-driven architecture is used to stream statuses (`recording`, `transcribing`, `success`, `error`) and the final `transcript` back to the frontend.
