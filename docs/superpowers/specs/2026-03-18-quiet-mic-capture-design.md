# Quiet Mic Capture Design

## Goal

Improve dictation reliability for quiet speakers by making microphone capture more tolerant of low-volume speech without making normal recordings noisier or harder to use.

## Scope

- Replace the fixed silence gate with configurable sensitivity.
- Add conservative automatic gain for clearly quiet recordings.
- Expose a simple mic sensitivity control in Settings.

## Non-goals

- Live level meters.
- Noise suppression or echo cancellation.
- Per-device microphone selection.

## Design

### Backend

The current backend computes clip RMS in `stop_recording_cmd` and rejects anything below a fixed threshold. That is too brittle because different microphones and speaking styles produce very different amplitudes. The new flow should:

1. Compute clip metrics from the captured samples.
2. Derive the silence threshold from a persisted `mic_sensitivity` setting.
3. Reject only clips that are effectively silent.
4. Apply bounded normalization only when a clip is quiet but still above the silence gate.
5. Encode the adjusted samples and continue with transcription.

Normalization should be conservative. It should target a modest peak level and cap gain so room noise is not amplified aggressively.

### Settings

Persist a new `mic_sensitivity` numeric field in `AppSettings`. The default should sit in the middle of the supported range so existing users get better behavior without needing to tune the app immediately.

### UI

Add a single slider under Settings that maps to the backend sensitivity value. The copy should make the tradeoff clear: higher sensitivity helps softer voices, but can pick up more background noise.

## Acceptance Criteria

- Quiet speech is no longer rejected by the previous fixed threshold.
- Clearly silent recordings still return "No voice detected".
- Quiet recordings are boosted before transcription using bounded gain.
- Users can tune mic sensitivity in Settings and save it.
- Existing settings files continue to load safely with a default sensitivity value.
