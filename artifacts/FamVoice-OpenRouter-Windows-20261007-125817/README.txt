FamVoice OpenRouter - local Windows x64 package, version 0.5.0

Launch the standalone EXE in this folder. The installers are optional; do not run
an installer if you want to leave the currently installed FamVoice unchanged.
Windows 10/11 x64 and Microsoft Edge WebView2 Runtime are required.

Setup:
1. Open FamVoice Settings.
2. Select OpenRouter and MAI Transcribe 2 (microsoft/mai-transcribe-2).
3. Enter your OpenRouter API key and select Portuguese.
4. Set the dictation shortcut to Mouse 5 using the shortcut capture control.
   The default shortcut is Ctrl+Shift+Space; Mouse 5 is not preconfigured.
5. Enable Auto Paste and save settings.
6. Focus the T3 message composer, hold Mouse 5 while speaking, then release it.
   Keep the same T3 window focused until transcription completes.
   Text is inserted without prompt optimization or automatically sending it.

This build keeps the existing app identifier (com.famvoice.desktop), so launching
it may reuse existing FamVoice settings/history. It is not a separate data profile.
No installed app was updated and no running app was stopped during packaging.

Verification boundary:
The package build and file integrity are checked. Native launch, microphone
capture, live OpenRouter transcription, and insertion into T3 are not tested.
The local binaries are not Authenticode signed; Windows may show a publisher warning.
This package is not a published updater release.

See BUILD-REPORT.md and SHA256SUMS.txt for build results and file integrity.
