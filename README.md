# FamVoice

A lightweight desktop dictation app. Hold a hotkey, speak, and the transcribed text is pasted directly into whatever window you're using.

Built with **Tauri v2** (Rust) + **React** + **Tailwind CSS**.

## How It Works

1. Press and hold a global hotkey (default `Ctrl+Shift+Space`)
2. Speak into your microphone
3. Release the hotkey
4. Your speech is transcribed via OpenAI and automatically pasted into the active window

No browser tabs, no copy-pasting, no switching windows. Just talk and it types.

## Features

- **Global Hotkey** - Works in any application, configurable shortcut
- **Instant Paste** - Transcribed text is injected directly into the focused input field
- **Clipboard Preservation** - Optionally restores your clipboard after pasting
- **Prompt Optimization** - Optional AI pass (Anthropic) that rewrites your dictation into a polished implementation prompt for coding agents
- **Glossary Replacements** - Auto-correct specific words or phrases (e.g. "omg" -> "Oh my gosh")
- **Widget Mode** - Minimal floating overlay showing only the recording waveform
- **History** - Browse, copy, or re-paste past transcriptions
- **Sound Cues** - Audio feedback for recording start, stop, success, and errors
- **Launch on Startup** - Auto-start with your OS
- **Mic Sensitivity Control** - Adjustable threshold to trim silence

## Supported Transcription Models

- `gpt-4o-mini-transcribe` (default)
- `gpt-4o-transcribe`
- `whisper-1`

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform
- An OpenAI API key (for transcription)
- *(Optional)* An Anthropic API key (for prompt optimization)

## Getting Started

```bash
# Clone the repo
git clone git@github.com:famintos/FamVoice.git
cd FamVoice

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

On first launch, open **Settings** and paste your OpenAI API key.

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
    anthropic.rs   Anthropic API client
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
| Prompt Optimization | Anthropic API (Claude) |
| Clipboard | arboard |
| Key Injection | enigo |
| Icons | Lucide React |
