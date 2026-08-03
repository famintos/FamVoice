[CmdletBinding()]
param(
    [ValidateSet('Preflight', 'Interactive')]
    [string]$Mode = 'Preflight',

    [Parameter(Mandatory = $true)]
    [string]$AppPath,

    [string]$PreviousInstaller,
    [string]$PreviousInstallerSignature,
    [string]$UpdaterMetadataUrl,
    [string]$ExpectedVersion,
    [string]$ReportPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

if (-not $ReportPath) {
    $ReportPath = Join-Path $PSScriptRoot '..\docs\windows-native-smoke-latest.md'
}

$results = [System.Collections.Generic.List[object]]::new()

function Add-SmokeResult {
    param(
        [string]$Id,
        [string]$Status,
        [string]$Evidence
    )

    $results.Add([pscustomobject]@{
        Id = $Id
        Status = $Status
        Evidence = ($Evidence -replace '[\r\n\t]+', ' ')
    })
}

function Read-SmokeResult {
    param(
        [string]$Id,
        [string]$Prompt
    )

    while ($true) {
        $answer = (Read-Host "$Prompt [pass/fail/skip]").Trim().ToLowerInvariant()
        if ($answer -in @('pass', 'fail', 'skip')) {
            $evidence = Read-Host 'Short evidence (no transcript, audio, or secrets)'
            Add-SmokeResult -Id $Id -Status $answer.ToUpperInvariant() -Evidence $evidence
            return
        }
    }
}

function Write-SmokeReport {
    param([string]$ResolvedReportPath)

    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add('# FamVoice Windows native smoke report')
    $lines.Add('')
    $lines.Add("- Date: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss K')")
    $lines.Add("- Mode: $Mode")
    $lines.Add("- App: $AppPath")
    $lines.Add("- Existing FamVoice process was terminated: no")
    $lines.Add('')
    $lines.Add('| Check | Result | Evidence |')
    $lines.Add('| --- | --- | --- |')
    foreach ($result in $results) {
        $evidence = $result.Evidence -replace '\|', '\|'
        $lines.Add("| $($result.Id) | $($result.Status) | $evidence |")
    }

    $reportDirectory = Split-Path -Parent $ResolvedReportPath
    if (-not (Test-Path -LiteralPath $reportDirectory)) {
        New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
    }
    [System.IO.File]::WriteAllLines($ResolvedReportPath, $lines)
}

$resolvedReportPath = [System.IO.Path]::GetFullPath($ReportPath)
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$resolvedAppPath = $null
$originalClipboard = $null
$hasClipboardSnapshot = $false
$launchedProcess = $null

try {
    if ($env:OS -ne 'Windows_NT') {
        throw 'The FamVoice native smoke only supports Windows.'
    }

    if (-not (Test-Path -LiteralPath $AppPath -PathType Leaf)) {
        Add-SmokeResult -Id 'app-artifact' -Status 'BLOCKED' -Evidence "App executable not found at $AppPath"
        throw "App executable not found at '$AppPath'."
    }

    $resolvedAppPath = (Resolve-Path -LiteralPath $AppPath).Path
    $appItem = Get-Item -LiteralPath $resolvedAppPath
    Add-SmokeResult -Id 'app-artifact' -Status 'PASS' -Evidence "Found $($appItem.Name), $($appItem.Length) bytes, version $($appItem.VersionInfo.FileVersion)"

    $existing = @(Get-CimInstance Win32_Process | Where-Object {
        $_.Name -ieq 'famvoice.exe'
    })
    if ($existing.Count -gt 0) {
        $ids = ($existing.ProcessId | Sort-Object) -join ', '
        Add-SmokeResult -Id 'exclusive-session' -Status 'BLOCKED' -Evidence "FamVoice already active (PID $ids); no process was terminated"
    } else {
        Add-SmokeResult -Id 'exclusive-session' -Status 'PASS' -Evidence 'No active FamVoice process detected'
    }

    $monitorCount = [System.Windows.Forms.SystemInformation]::MonitorCount
    Add-SmokeResult -Id 'monitors' -Status 'PASS' -Evidence "$monitorCount monitor(s) detected"

    if ($PreviousInstaller) {
        if (-not (Test-Path -LiteralPath $PreviousInstaller -PathType Leaf)) {
            Add-SmokeResult -Id 'previous-installer' -Status 'BLOCKED' -Evidence 'Previous installer path does not exist'
        } else {
            $authenticode = Get-AuthenticodeSignature -LiteralPath $PreviousInstaller
            if ($authenticode.Status -eq 'Valid') {
                $signer = $authenticode.SignerCertificate.Subject
                Add-SmokeResult -Id 'previous-installer' -Status 'PASS' -Evidence "Authenticode signature valid; signer: $signer"
            } elseif ($PreviousInstallerSignature) {
                if (-not (Test-Path -LiteralPath $PreviousInstallerSignature -PathType Leaf)) {
                    Add-SmokeResult -Id 'previous-installer' -Status 'BLOCKED' -Evidence 'Detached updater signature path does not exist'
                } elseif (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
                    Add-SmokeResult -Id 'previous-installer' -Status 'BLOCKED' -Evidence 'Cargo is required to verify the detached Tauri updater signature'
                } else {
                    $tauriConfig = Join-Path $repoRoot 'src-tauri\tauri.conf.json'
                    $savedErrorActionPreference = $ErrorActionPreference
                    $ErrorActionPreference = 'Continue'
                    try {
                        $verificationOutput = @(& cargo run --quiet --manifest-path (Join-Path $repoRoot 'src-tauri\Cargo.toml') --example verify_updater_signature -- $PreviousInstaller $PreviousInstallerSignature $tauriConfig 2>&1)
                        $verificationExitCode = $LASTEXITCODE
                    } finally {
                        $ErrorActionPreference = $savedErrorActionPreference
                    }
                    $verificationEvidence = ($verificationOutput | ForEach-Object { $_.ToString() }) -join ' '
                    if ($verificationExitCode -eq 0) {
                        Add-SmokeResult -Id 'previous-installer' -Status 'PASS' -Evidence "Tauri updater signature valid; Authenticode: $($authenticode.Status); $verificationEvidence"
                    } else {
                        Add-SmokeResult -Id 'previous-installer' -Status 'FAIL' -Evidence "Tauri updater signature invalid; $verificationEvidence"
                    }
                }
            } else {
                Add-SmokeResult -Id 'previous-installer' -Status 'FAIL' -Evidence "Authenticode status: $($authenticode.Status); no detached Tauri updater signature supplied"
            }
        }
    } else {
        Add-SmokeResult -Id 'previous-installer' -Status 'SKIP' -Evidence 'No previous installer supplied'
    }

    if ($UpdaterMetadataUrl) {
        try {
            $metadata = Invoke-RestMethod -Uri $UpdaterMetadataUrl -Method Get
            $metadataVersion = [string]$metadata.version
            $metadataStatus = if ($ExpectedVersion -and $metadataVersion -ne $ExpectedVersion) { 'FAIL' } else { 'PASS' }
            Add-SmokeResult -Id 'updater-metadata' -Status $metadataStatus -Evidence "Published updater version: $metadataVersion"
        } catch {
            Add-SmokeResult -Id 'updater-metadata' -Status 'FAIL' -Evidence $_.Exception.Message
        }
    } else {
        Add-SmokeResult -Id 'updater-metadata' -Status 'SKIP' -Evidence 'No updater metadata URL supplied'
    }

    if ($Mode -eq 'Preflight') {
        return
    }

    if ($existing.Count -gt 0) {
        throw 'Interactive smoke refused because an existing FamVoice process may own the hotkey, microphone, clipboard, or app data.'
    }

    try {
        $originalClipboard = Get-Clipboard -Raw
        $hasClipboardSnapshot = $true
        Add-SmokeResult -Id 'clipboard-snapshot' -Status 'PASS' -Evidence 'Original text clipboard captured for restoration'
    } catch {
        Add-SmokeResult -Id 'clipboard-snapshot' -Status 'FAIL' -Evidence $_.Exception.Message
    }

    $unicodeLetters = -join @([char]0x00E7, [char]0x00E3, [char]0x00F5)
    $emoji = [char]::ConvertFromUtf32(0x1F604)
    $clipboardSentinel = "FamVoice smoke sentinel - $unicodeLetters`r`nSecond line $emoji"
    Set-Clipboard -Value $clipboardSentinel
    Add-SmokeResult -Id 'clipboard-sentinel' -Status 'PASS' -Evidence 'Synthetic Unicode multiline sentinel placed on clipboard'

    $launchedProcess = Start-Process -FilePath $resolvedAppPath -PassThru
    Start-Sleep -Milliseconds 1200
    if ($launchedProcess.HasExited) {
        Add-SmokeResult -Id 'launch' -Status 'FAIL' -Evidence "FamVoice exited with code $($launchedProcess.ExitCode)"
        throw 'FamVoice exited during launch.'
    }
    Add-SmokeResult -Id 'launch' -Status 'PASS' -Evidence "Started PID $($launchedProcess.Id) without replacing another session"

    Read-SmokeResult -Id 'settings' -Prompt 'Open Settings from the UI/tray; confirm it opens and closes without focus loss'
    Read-SmokeResult -Id 'hotkey-dictation' -Prompt 'In an external editor, dictate synthetic Unicode multiline text through the global hotkey and verify exact delivery'
    Read-SmokeResult -Id 'clipboard-unicode' -Prompt 'Verify clipboard copy/preserve behavior using the synthetic sentinel and Unicode multiline text'
    Read-SmokeResult -Id 'tray-hide-restore' -Prompt 'Hide the widget, restore it from tray, then repeat with the global hotkey'
    Read-SmokeResult -Id 'window-close-recovery' -Prompt 'Use the widget close control and confirm tray/hotkey recovery without stealing focus'
    Read-SmokeResult -Id 'monitor-clamp' -Prompt 'Move the widget to another monitor or screen edge, change monitor availability, and verify it returns on-screen'
    Read-SmokeResult -Id 'history-repaste' -Prompt 'Re-paste Unicode multiline history twice and confirm the original clipboard is restored'
    Read-SmokeResult -Id 'retry-last-dictation' -Prompt 'With synthetic speech, force a provider/network failure, recover it, then Retry without speaking again; confirm exactly one paste'
    Read-SmokeResult -Id 'retry-privacy-lifecycle' -Prompt 'Create another failed synthetic dictation, then verify Discard, expiry, a new recording, and app restart each remove Retry without creating an audio file'
    Read-SmokeResult -Id 'diagnostics-microphone-device' -Prompt 'In Diagnostics, test the microphone level, then disconnect/reconnect the selected device and confirm the state updates'
    Read-SmokeResult -Id 'diagnostics-hotkey-provider' -Prompt 'Validate the saved hotkey, a real conflicting shortcut, and the authenticated provider test without sending speech'
    Read-SmokeResult -Id 'diagnostics-export-privacy' -Prompt 'Export diagnostics and inspect the JSON; confirm it contains no API key, transcript, glossary term, audio, or device identifier'
    Read-SmokeResult -Id 'history-search-pin-export' -Prompt 'Search synthetic history, pin/unpin an entry, export TXT/Markdown/JSON explicitly, and verify each export contains only the expected synthetic entries'
    Read-SmokeResult -Id 'history-retention-purge' -Prompt 'Change retention, add synthetic entries, then Delete all; restart and confirm current, backup, and recovery history remain empty'
    Read-SmokeResult -Id 'updater-ui' -Prompt 'Open the updater UI and verify the published state/version without installing an unapproved build'
    if ($PreviousInstaller -and $UpdaterMetadataUrl -and $ExpectedVersion) {
        Read-SmokeResult -Id 'signed-upgrade' -Prompt 'Using the supplied signed previous installer, perform the approved install/update and verify the final version'
    } else {
        Add-SmokeResult -Id 'signed-upgrade' -Status 'SKIP' -Evidence 'Signed previous installer, updater URL, and expected newer version were not all supplied'
    }

    Read-SmokeResult -Id 'clean-exit' -Prompt 'Exit FamVoice through the tray menu; the script will not terminate it'
} finally {
    if ($hasClipboardSnapshot) {
        Set-Clipboard -Value $originalClipboard
    }

    if ($launchedProcess -and -not $launchedProcess.HasExited) {
        Add-SmokeResult -Id 'process-left-running' -Status 'WARN' -Evidence "PID $($launchedProcess.Id) is still active and was not terminated"
    }

    Write-SmokeReport -ResolvedReportPath $resolvedReportPath
    Write-Output "Native smoke report: $resolvedReportPath"
}
