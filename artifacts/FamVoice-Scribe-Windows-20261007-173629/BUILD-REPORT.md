# FamVoice Scribe v2 Windows build

Built on 2026-10-07. Version 0.5.0, Windows x64.

## Try it

1. Exit the currently running FamVoice from its tray icon.
2. Open `FamVoice-Scribe-0.5.0-x64.exe` or `FamVoice Scribe.lnk` in this folder.
3. In Settings, choose **OpenRouter**, then **Scribe v2** as the transcription model.
4. Leave the API key input blank to reuse the saved OpenRouter key. Select
   **Auto Detect** to try Portuguese mixed with English, then save changes.
5. Dictate with your existing hotkey. MAI Transcribe 2 remains available in the
   same picker, and remains selected until you choose Scribe.

This standalone executable uses FamVoice's existing application identifier and
settings. It was not installed or launched. The running app and previous package
were not modified or stopped.

## Change and verification

- Added `elevenlabs/scribe-v2` to the frontend picker and backend model allowlist.
- Saved Scribe selections survive reload and share the existing OpenRouter key.
- Scribe uses `provider.options.elevenlabs`, not MAI's Azure options. The normal
  completed-clip, replacement, and paste flow is preserved.
- Requests disable speaker labels, audio-event tags, timestamps, and transcript
  editing. Valid glossary targets use `keyterms`, which may incur a provider
  surcharge. Empty guidance is omitted. Prompt optimization remains disabled.
- `npm run test:components -- src/SettingsView.component.test.tsx`: 17 passed.
- `cargo test --lib openrouter --locked --jobs 1`: 14 passed.
- `rustfmt --edition 2021 --check src-tauri/src/settings.rs src-tauri/src/transcription/openrouter.rs`: passed.
- `npx eslint src/appConstants.ts src/SettingsView.component.test.tsx`: passed.
- `npm run tauri -- build --no-bundle --ci -- --locked --jobs 1`: exit 0;
  TypeScript/Vite and the Windows release build passed. Native compilation took
  3m 52s. The five existing unused-variable warnings remain.

The native build reused the isolated output directory
`src-tauri/target/openrouter-package-20261007-125817`. No installers, signed
updater artifacts, releases, commits, or pushes were produced. Tauri's temporary
Cargo.toml line-ending normalization was restored. All tracked files outside
this task's four edited files match the pre-task hashes; the OpenRouter transport
was an existing untracked file and was also updated for this task.

No authenticated transcription request, native launch, microphone recording, or
live paste was tested. Automated request tests establish the intended payload,
not the user's recognition accuracy or end-to-end latency.

## Artifact

- File: `FamVoice-Scribe-0.5.0-x64.exe`
- Size: 16,561,152 bytes
- PE architecture: x64, machine 0x8664
- SHA256: `750623C8354BB83C11FED18E445EDB6D432CA6452B28172A16681A72CC6B4BEB`
- Authenticode: NotSigned
- Copy SHA256 matches the compiled executable.

As checked in OpenRouter's endpoint catalog on 2026-10-07, Scribe v2's base
promotional rate is $0.11 per audio hour with a 50% discount. Pricing can change.
The catalog snapshot is saved in `scribe-endpoint-catalog.json`.

Evidence: `native-build.log`, `native-build-exit.txt`, `verification.json`,
`package-manifest.json`, `source-baseline.json`, `source-preservation.json`.
