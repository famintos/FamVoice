# FamVoice OpenRouter local Windows package

Version: 0.5.0. Architecture: Windows x64, PE Machine 0x8664.
Repository: C:\Users\henri\Desktop\Faminto\FamVoice
Artifact directory: C:\Users\henri\Desktop\Faminto\FamVoice\artifacts\FamVoice-OpenRouter-Windows-20261007-125817

## Exact commands and outcomes

Both commands used this separate native output directory:

```powershell
$env:CARGO_TARGET_DIR = 'C:\Users\henri\Desktop\Faminto\FamVoice\src-tauri\target\openrouter-package-20261007-125817'
$env:TAURI_SIGNING_PRIVATE_KEY = $null
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $null
npm run tauri -- build
```

Exit status: 1. The native release compiled successfully in 5m 02s and both
installers were generated. The final updater-artifact signing step failed:
"A public key has been found, but no private key."
No signing credentials were accessed. The frontend build ran through Tauri's
configured beforeBuildCommand; previously completed tests were not rerun.

Fix round 1 used Tauri's documented bundle command for an already-built app:

```powershell
npm run tauri -- bundle --config C:\Users\henri\Desktop\Faminto\FamVoice\artifacts\FamVoice-OpenRouter-Windows-20261007-125817\tauri.local-package.json --no-sign
```

Exit status: 0. Both MSI and NSIS installers generated successfully.
The task-local configuration is exactly:

```json
{"bundle":{"createUpdaterArtifacts":false}}
```

This only disables creation of signed updater artifacts for local packaging.
The repository's Tauri configuration, updater endpoint/public key, and app
identifier were not changed. No second fix round was necessary.

## Deliverables
- File: C:\Users\henri\Desktop\Faminto\FamVoice\artifacts\FamVoice-OpenRouter-Windows-20261007-125817\FamVoice-OpenRouter-0.5.0-x64.exe
  - Native build output: C:\Users\henri\Desktop\Faminto\FamVoice\src-tauri\target\openrouter-package-20261007-125817\release\famvoice.exe
  - Size: 16551936 bytes
  - SHA256: 6D8350EAF1AED05953B133A0AC454A15E42493265401B65DB2F5C3A73F3607BB
  - Authenticode: NotSigned

- File: C:\Users\henri\Desktop\Faminto\FamVoice\artifacts\FamVoice-OpenRouter-Windows-20261007-125817\FamVoice_0.5.0_x64-setup.exe
  - Native build output: C:\Users\henri\Desktop\Faminto\FamVoice\src-tauri\target\openrouter-package-20261007-125817\release\bundle\nsis\FamVoice_0.5.0_x64-setup.exe
  - Size: 4663936 bytes
  - SHA256: 88FA22E156A2B400E8658916921CC3ADB0B1863504357845C847CDBE43B51108
  - Authenticode: NotSigned

- File: C:\Users\henri\Desktop\Faminto\FamVoice\artifacts\FamVoice-OpenRouter-Windows-20261007-125817\FamVoice_0.5.0_x64_en-US.msi
  - Native build output: C:\Users\henri\Desktop\Faminto\FamVoice\src-tauri\target\openrouter-package-20261007-125817\release\bundle\msi\FamVoice_0.5.0_x64_en-US.msi
  - Size: 6475776 bytes
  - SHA256: B1DE35BCB712E9B5ED38632F06344ADF59223B680C27038CFA0B7E0766BB1BA9
  - Authenticode: NotSigned
A local shortcut, FamVoice OpenRouter.lnk, points to the standalone EXE in this
artifact folder. It was created without launching the app or modifying the desktop.
Copies were checked against the corresponding build outputs by SHA256.
The EXE's FileVersion and ProductVersion both read 0.5.0.
The EXE imports Windows system/UCRT DLLs; no app-local DLL imports were found.
The Tauri configuration declares no external resources or sidecar programs.
The frontend and icons are embedded. Windows 10/11 x64 with WebView2 Runtime
is still required; this is not a fully self-contained WebView2 distribution.

## Warnings, scope, and proof boundary

- Five compiler warnings: unused variables in src-tauri/src/audio.rs:184 and
  src-tauri/src/settings.rs:705, 717, 727, 851. No implementation edits were made.
- All three deliverables are unsigned (NotSigned); Windows may show an unknown
  publisher/SmartScreen warning. This is a local package, not an updater release.
- Tauri normalized Cargo.toml line endings. CRLF restoration matched the exact
  initial SHA256, preserving its preexisting dependency changes.
- Final hashes of every tracked file match the pre-build baseline, including
  deleted files. Existing source modifications were preserved.
- No commits, pushes, PRs, publishing, installations, updates, process termination,
  live app settings/credential reads, audio capture, or audio/API requests occurred.
- Native launch, microphone behavior, real OpenRouter transcription, and insertion
  into T3 were not tested. A successful package does not prove those interactions.
- The standalone app retains com.famvoice.desktop and can reuse existing FamVoice
  settings/history when the user launches it. Its build output is isolated; its
  runtime data profile is not isolated.

## Launch and setup

1. Open the standalone EXE or the local shortcut. Installers are optional and were
   not run. Use the standalone EXE to leave the existing installation untouched.
2. Open Settings, select OpenRouter and MAI Transcribe 2
   (microsoft/mai-transcribe-2), enter your OpenRouter API key, and select Portuguese.
3. Capture Mouse 5 as the dictation shortcut, enable Auto Paste, and save settings.
   The default shortcut is Ctrl+Shift+Space, not Mouse 5.
4. Focus the T3 composer, hold Mouse 5 while speaking, and release it to transcribe.
   Keep the same window focused until completion. The transcript is inserted
   verbatim after configured replacements, without prompt optimization or auto-send.

Evidence files: native-build.log, native-build-exit.txt, local-bundle.log,
local-bundle-exit.txt, package-manifest.json, SHA256SUMS.txt,
exe-runtime-imports.json, source-preservation-final.json, README.txt.