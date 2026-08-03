# FamVoice Windows native smoke report

- Date: 2026-08-02 14:27:39 +01:00
- Mode: Preflight
- App: src-tauri/target/release/famvoice.exe
- Existing FamVoice process was terminated: no

| Check | Result | Evidence |
| --- | --- | --- |
| app-artifact | PASS | Found famvoice.exe, 16101376 bytes, version 0.3.28 |
| exclusive-session | BLOCKED | FamVoice already active (PID 755124); no process was terminated |
| monitors | PASS | 2 monitor(s) detected |
| previous-installer | SKIP | No previous installer supplied |
| updater-metadata | SKIP | No updater metadata URL supplied |
