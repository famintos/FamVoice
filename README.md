# FamVoice

A lightweight Windows desktop dictation app. Hold a hotkey, speak, and the transcribed text is pasted directly into whatever window you're using.

Built with **Tauri v2** (Rust) + **React** + **Tailwind CSS**.

## Supported Platform

FamVoice is officially **Windows-only** in the current release line (Windows 10/11, x64). CI, signed installers, updater metadata, credential storage, and encrypted transcript history are all maintained for Windows. macOS and Linux builds are not supported because they do not yet have equivalent secure persistence and native smoke coverage.

## How It Works

1. Press and hold a global hotkey (default `Ctrl+Shift+Space`)
2. Speak into your microphone
3. Release the hotkey
4. Your speech is transcribed via OpenAI or Groq and automatically pasted into the active window

No browser tabs, no copy-pasting, no switching windows. Just talk and it types.

## Features

- **Global Hotkey** - Works in any application, configurable shortcut
- **Instant Paste** - Transcribed text is injected directly into the focused input field
- **Clipboard Copy** - Optionally keeps the final transcript on your clipboard after you finish speaking
- **Bring Your Own Keys** - FamVoice is a local desktop client; you provide your own OpenAI and Groq API keys
- **Prompt Optimization** - Optional AI pass (OpenAI GPT-5.4 Mini) that rewrites your dictation into a polished implementation prompt for coding agents
- **Glossary Replacements** - Auto-correct specific words or phrases (e.g. "omg" -> "Oh my gosh")
- **Widget Mode** - Minimal floating overlay showing only the recording waveform
- **Recover Failed Dictation** - Retry the most recent failed upload for up to two minutes without speaking again
- **Diagnostics** - Test microphone signal, device/hotkey state, and provider authentication without sending dictated content
- **History** - Search, pin, copy, re-paste, export, or delete past transcriptions
- **Sound Cues** - Audio feedback for recording start, stop, success, and errors
- **Launch on Startup** - Auto-start with your OS
- **Mic Sensitivity Control** - Adjustable threshold to trim silence

## Supported Transcription Models

- `gpt-transcribe` (OpenAI, recommended default for completed-file dictation; $0.0045/min)
- `whisper-1` (OpenAI, specialized fallback for timestamps, subtitles, or translation; $0.006/min)
- `whisper-large-v3-turbo` (Groq, speed / value; $0.04/hour)
- `whisper-large-v3` (Groq, accuracy-first; $0.111/hour)

Existing Groq choices are preserved. Legacy unversioned OpenAI settings migrate once to `gpt-transcribe`; after the migration, an explicit `whisper-1` choice remains stable. See [Transcription models](docs/transcription-models.md) for the provider field matrix, versioned migration policy, official sources, and the dated pt-PT evaluation status.

## Supported Prompt Optimization Models

- `gpt-5.4-mini` (default)

## Prerequisites

- Windows 10 or Windows 11 (x64)
- [Node.js](https://nodejs.org/) (v20.19+ on Node 20, v22.13+ on Node 22, or v24+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for Windows
- An OpenAI API key or Groq API key (for transcription)
- *(Optional)* An OpenAI API key (for prompt optimization)

## Privacy And Keys

FamVoice does not ship with a shared backend API key. It runs as a local desktop client and uses the provider keys you enter in **Settings**.

- OpenAI key: required when transcription provider is `OpenAI`
- Groq key: required when transcription provider is `Groq`
- Prompt optimization key: optional OpenAI API key, only used when prompt optimization is enabled

API keys are stored in Windows Credential Manager with a DPAPI-encrypted local recovery copy. They are not committed to the repo and are never written to settings in plaintext.

Transcript history is DPAPI-encrypted and bounded to at most 100 items. Settings can reduce the limit or stop saving new transcripts; **Clear history** purges the active history plus FamVoice recovery copies. Explicit TXT, Markdown, and JSON exports are plaintext files in Downloads and remain under the user's control.

When an upload fails after valid speech was captured, FamVoice can retain only that one encoded recording in process RAM for at most two minutes and 10 MiB. Retry consumes it once. Expiry, discard, a new recording, or closing FamVoice removes it; audio is never added to logs, transcript history, diagnostics, or a temporary file.

## Getting Started

```bash
# Clone the repo
git clone https://github.com/famintos/FamVoice.git
cd FamVoice

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

On first launch, open **Settings**, choose your transcription provider, and paste the corresponding API key.

## Auto-Update

The shipped updater is configured to read release artifacts from the public GitHub Releases feed for this repository. If you publish a fork or move the code to a private repository, update the Tauri updater endpoint before shipping or auto-update will either break or point at the upstream FamVoice releases.

## Architecture

```
src/              React + TypeScript frontend (single-page App.tsx)
src-tauri/src/
  lib.rs          Core app logic, IPC commands, hotkey handling
  audio.rs        Microphone capture via cpal (16kHz mono, silence trimming)
  transcription.rs  OpenAI API integration
  clipboard.rs    System clipboard read/write (arboard)
  injection.rs    Keystroke simulation for auto-paste (enigo)
  settings.rs     Atomic settings and Windows credential persistence
  history.rs      Atomic DPAPI-encrypted transcript history
  retry_audio.rs  Single-use, RAM-only failed-audio recovery cache
  diagnostics.rs  Sanitized device, hotkey, provider and latency diagnostics
  user_export.rs  Explicit non-overwriting exports to Downloads
  persistence.rs  Shared atomic-write and recovery primitive
  prompt_optimizer/
    mod.rs         Prompt optimization orchestration
    openai.rs      OpenAI API client
    metaprompt.rs  System instruction for prompt rewriting
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for more details.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 |
| Backend | Rust |
| Frontend | React 19, TypeScript, Tailwind CSS 4 |
| Audio | cpal |
| Transcription | OpenAI (`gpt-transcribe`, `whisper-1`) or Groq Whisper API |
| Prompt Optimization | OpenAI API (GPT-5.4 Mini) |
| Clipboard | arboard |
| Key Injection | enigo |
| Icons | Lucide React |
