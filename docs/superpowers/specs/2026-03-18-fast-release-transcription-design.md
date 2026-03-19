# Fast Release Transcription Design

## Goal

Reduce the delay between releasing the push-to-talk hotkey and seeing the final transcript pasted into the active text field.

## Scope

- Keep the current push-to-talk interaction: press to talk, release to paste.
- Keep transcript insertion final-only, with no partial or draft text shown to the user.
- Shift transcription work earlier by streaming audio to OpenAI while recording is still in progress.
- Preserve the existing clipboard and paste behavior after the final transcript is available.
- Keep a safe fallback to the current upload-after-release pipeline.

## Non-goals

- Showing partial transcripts in the UI or target application.
- Changing the paste mechanism or target-app behavior.
- Supporting every existing transcription model in the first fast-release implementation.
- Reworking device selection, live audio monitoring, or UI layout.

## Model Choice

The fast-release path should target `gpt-4o-mini-transcribe`.

This is the right initial constraint because the goal is lower post-release latency, not maximum model flexibility. Fixing the first implementation to one model keeps the backend state machine smaller and reduces compatibility risk while the new pipeline is being proven.

The existing non-streaming transcription path can remain available as a fallback path and compatibility escape hatch.

## Design

### Current Problem

FamVoice currently waits until release before it starts the expensive part of the workflow:

1. Stop recording.
2. Encode the full clip to WAV.
3. Upload the whole clip to `/v1/audio/transcriptions`.
4. Wait for the full response.
5. Copy the text to the clipboard and paste it.

That means network transfer and model inference sit entirely after the user releases the hotkey, which is exactly where the latency is most visible.

### Proposed Flow

The new flow should overlap network and transcription work with the time the user is still speaking:

1. `start_recording_cmd` starts local recording as it does today.
2. At the same time, the backend opens a Realtime transcription session configured for `gpt-4o-mini-transcribe`.
3. While the user is holding the hotkey, captured PCM audio is forwarded to the Realtime session in small chunks.
4. The backend may receive partial transcript events, but it does not surface them to the UI or paste target.
5. On hotkey release, `stop_recording_cmd` stops capture and finalizes the active Realtime input.
6. The backend waits only for the final transcript completion event for that turn.
7. The transcript then goes through the existing post-processing path: replacements, trailing-period cleanup, clipboard write, paste, optional clipboard restore, and history insertion.

This keeps the visible UX unchanged while moving most of the costly work before the release event.

### Backend State

Add a dedicated `RealtimeTranscriptionState` separate from `AudioState`.

`AudioState` should remain responsible for local capture and fallback buffering. `RealtimeTranscriptionState` should own:

- connection/session lifecycle
- outgoing audio chunk submission
- final transcript collection
- session health and fallback eligibility

This separation prevents audio capture concerns from becoming tightly coupled to network protocol concerns.

### Audio Pipeline Requirements

The backend must keep a local audio buffer even while streaming to Realtime.

That local buffer is required for fallback if:

- the Realtime session fails to open
- the Realtime connection drops mid-recording
- the final transcript does not arrive in time
- the API returns an unrecoverable session error

The same captured PCM can therefore serve two purposes:

- low-latency streaming to Realtime during recording
- full-clip fallback upload after release when necessary

### Final-Only Transcript Behavior

The UI and injection flow should remain final-only.

Realtime partials may be logged or held internally for diagnostics, but they should not be emitted through the current `transcript` event and should not be pasted into the active application. The app should continue to emit success only when the finalized transcript is ready for insertion.

### Fallback Strategy

Fallback behavior should be explicit and predictable:

1. If Realtime setup fails at recording start, keep recording locally and mark the turn for fallback.
2. If Realtime fails during recording, keep buffering locally and mark the turn for fallback.
3. If finalization succeeds and a final transcript arrives in time, use the Realtime result.
4. If finalization fails or times out, encode the local buffer and call the current `/v1/audio/transcriptions` path.

This ensures the new architecture improves latency without reducing reliability.

### Settings and Compatibility

The first version of fast-release transcription should run only when the selected model is `gpt-4o-mini-transcribe`.

If the user chooses another model, the app can either:

- fall back automatically to the existing non-streaming path, or
- disable fast-release mode for that turn

The simpler first implementation is automatic fallback, because it avoids new UI state while preserving expected behavior.

## Error Handling

- Realtime connection errors should be logged with enough detail to distinguish setup, stream, and finalize failures.
- The user-facing error path should stay simple and reuse the current error/status events where possible.
- A failed Realtime turn must not lose the user’s spoken audio if local capture succeeded.
- Timeouts on final transcript completion should trigger fallback rather than immediate hard failure.

## Testing

- Add unit tests for Realtime session state transitions: created, streaming, finalized, failed, fallback.
- Add tests that verify `stop_recording_cmd` prefers the finalized Realtime transcript when available.
- Add tests that verify the legacy `/v1/audio/transcriptions` path is used when Realtime is unavailable or times out.
- Manually test short and medium dictations and compare release-to-paste latency against the current implementation.
- Manually verify that no partial transcript is shown or pasted before finalization.

## Acceptance Criteria

- Releasing the hotkey no longer starts the full transcription workload from scratch in the common case.
- The app streams audio to OpenAI while the user is still speaking.
- The pasted text still appears only once, as final text.
- Realtime failures fall back to the current full-clip upload flow without losing the recording.
- `gpt-4o-mini-transcribe` is the supported fast-release model for the first implementation.
