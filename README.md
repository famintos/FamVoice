# FamVoice

A lightweight desktop dictation app. Hold a hotkey, speak, and the transcribed text is pasted directly into whatever window you're using.

Built with **Tauri v2** (Rust) + **React** + **Tailwind CSS**.

## How It Works

1. Press and hold a global hotkey (default `Ctrl+Shift+Space`)
2. Speak into your microphone
3. Release the hotkey
4. Your speech is transcribed via OpenAI or Groq and automatically pasted into the active window

No browser tabs, no copy-pasting, no switching windows. Just talk and it types.

## Features

- **Global Hotkey** - Works in any application, configurable shortcut
- **Instant Paste** - Transcribed text is injected directly into the focused input field
- **Clipboard Preservation** - Optionally restores your clipboard after pasting
- **Bring Your Own Keys** - FamVoice is a local desktop client; you provide your own OpenAI and Groq API keys
- **Prompt Optimization** - Optional AI pass (OpenAI GPT-5.4 Mini) that rewrites your dictation into a polished implementation prompt for coding agents
- **Glossary Replacements** - Auto-correct specific words or phrases (e.g. "omg" -> "Oh my gosh")
- **Widget Mode** - Minimal floating overlay showing only the recording waveform
- **History** - Browse, copy, or re-paste past transcriptions
- **Sound Cues** - Audio feedback for recording start, stop, success, and errors
- **Launch on Startup** - Auto-start with your OS
- **Mic Sensitivity Control** - Adjustable threshold to trim silence

## Supported Transcription Models

- `whisper-1` (OpenAI, legacy / fallback)
- `whisper-large-v3-turbo` (Groq)
- `whisper-large-v3` (Groq)

## Supported Prompt Optimization Models

- `gpt-5.4-mini` (default)

## Prerequisites

- [Node.js](https://nodejs.org/) (v20.19+ or v22.12+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform
- An OpenAI API key or Groq API key (for transcription)
- *(Optional)* An OpenAI API key (for prompt optimization)

## Privacy And Keys

FamVoice does not ship with a shared backend API key. It runs as a local desktop client and uses the provider keys you enter in **Settings**.

- OpenAI key: required when transcription provider is `OpenAI`
- Groq key: required when transcription provider is `Groq`
- Prompt optimization key: optional OpenAI API key, only used when prompt optimization is enabled

API keys are stored in your OS credential store / keyring, not committed to the repo and not intended to live in plaintext project files.

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
  settings.rs     JSON settings persistence
  history.rs      Transcript history log
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
| Transcription | OpenAI API (Whisper / GPT-4o) |
| Prompt Optimization | OpenAI API (GPT-5.4 Mini) |
| Clipboard | arboard |
| Key Injection | enigo |
| Icons | Lucide React |
