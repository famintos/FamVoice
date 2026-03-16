# FamVoice

FamVoice is a lightweight, global desktop application built with **Tauri v2**, **React**, and **Rust**. It provides seamless audio recording, fast transcription via OpenAI's APIs, and automatic clipboard replacement and injection directly into your active window.

## Features
- **Global Hotkey Support:** Hold a configurable hotkey (e.g., `CommandOrControl+Shift+Space`) to record anywhere.
- **Fast Transcription:** Streams audio directly to OpenAI's Whisper or GPT-4o-mini models.
- **Auto-Paste:** Automatically pastes the transcribed text into your currently focused application.
- **Clipboard Management:** Optionally preserves your original clipboard content.
- **Phrase Replacements:** Configure automatic text replacements (e.g., "omg" -> "Oh my gosh") applied to every transcription.
- **History Log:** View past transcripts, copy them again, or quickly re-paste them anywhere.
- **Sound Cues:** Intuitive auditory feedback on recording start, stop, success, and error.
- **Launch on Startup:** Automatically starts with your operating system for immediate access.

## Architecture
See [ARCHITECTURE.md](ARCHITECTURE.md) for details.

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```
