# FamVoice Windows native smoke

This smoke is intentionally separate from unit/component tests. It exercises real Windows behavior that jsdom and Rust mocks cannot prove: tray recovery, global hotkeys, focus-preserving show/hide, monitor clamping, clipboard restoration, Unicode multiline delivery, retry lifecycle, microphone/device diagnostics, privacy-safe exports, history retention/purge, and the signed updater path.

## Safety rules

- Run only when no FamVoice process is active. The script refuses to launch over an existing installed or development instance.
- Do not force-close FamVoice. End the run through the tray Exit action.
- Use synthetic dictated text only. Do not include personal transcripts or API keys in the report.
- Inspect exported files locally, but record only pass/fail and content categories in the report. Never paste exported transcript text, device identifiers, paths containing a username, or secret-shaped values into the evidence field.
- The upgrade lane is opt-in and requires explicit paths to signed artifacts.

## Preflight

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows-native-smoke.ps1 `
  -Mode Preflight `
  -AppPath src-tauri/target/release/famvoice.exe
```

Preflight records process conflicts, executable version/signature, monitor count and updater metadata availability without starting the app.

## Interactive native smoke

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows-native-smoke.ps1 `
  -Mode Interactive `
  -AppPath src-tauri/target/release/famvoice.exe
```

The operator records pass/fail/skip for each prompted check. The script creates `docs/windows-native-smoke-latest.md` and restores the original text clipboard in `finally`.

The Phase 6 checks deliberately separate feature proof:

- Retry proves recovery, single delivery, discard, expiry, replacement by a new recording, and RAM-only lifetime across restart.
- Diagnostics proves microphone signal, device disconnect/reconnect, hotkey availability/conflict, authenticated provider access without speech, and a redacted export.
- History proves local search, persistent pins, explicit TXT/Markdown/JSON export, retention enforcement, and purge across restart/recovery copies.
- Long/realtime dictation is not part of the product smoke until its benchmark records a go decision; the current push-to-talk path remains the baseline.

## Signed installation/upgrade lane

Use only with a previous signed installer and an updater endpoint for a newer, published version:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows-native-smoke.ps1 `
  -Mode Interactive `
  -AppPath 'C:/Program Files/FamVoice/famvoice.exe' `
  -PreviousInstaller artifacts/FamVoice_0.3.29_x64-setup.exe `
  -PreviousInstallerSignature artifacts/FamVoice_0.3.29_x64-setup.exe.sig `
  -UpdaterMetadataUrl 'https://github.com/famintos/FamVoice/releases/latest/download/latest.json' `
  -ExpectedVersion 0.3.30
```

The script accepts either valid Authenticode or the detached Tauri updater signature. For the Tauri path it verifies the artifact against the public key in `src-tauri/tauri.conf.json`, using the same Minisign policy as `tauri-plugin-updater`, then checks the updater metadata before asking the operator to perform the install/update. It never downloads, installs, kills, tags or publishes anything by itself.
