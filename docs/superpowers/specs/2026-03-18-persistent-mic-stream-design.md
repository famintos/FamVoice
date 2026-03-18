# Persistent Mic Stream Design

## Goal

Reduce push-to-talk startup latency so microphone capture begins effectively immediately after the hotkey press is recognized.

## Scope

- Keep one microphone input stream open while FamVoice is running.
- Move stream creation and startup out of the hotkey press path.
- Preserve the existing press-to-record, release-to-transcribe interaction model.
- Recover cleanly if the persistent stream becomes invalid after startup.

## Non-goals

- Changing transcription, clipboard, or paste behavior.
- Adding device selection, live meters, or noise suppression.
- Hiding the OS microphone privacy indicator while the app is open.

## Design

### Backend

The current audio pipeline rebuilds the input device and `cpal::Stream` on every recording start. That work sits directly on the hotkey path and creates a small but noticeable delay before samples can be captured.

The new backend flow should keep a single input stream alive inside `AudioState` for the lifetime of the app:

1. Resolve the default input device and its preferred config when `AudioState` is created.
2. Build the `cpal` input stream once and call `play()` once during initialization.
3. Keep the callback active even while the app is idle.
4. Gate recording with an `armed` flag that determines whether incoming callbacks should append samples to the shared buffer.
5. Clear the sample buffer and set `armed = true` when recording starts.
6. Set `armed = false` and return the accumulated samples when recording stops.

While idle, the callback should return before downmixing, filtering, resampling, or appending samples. That keeps the steady-state overhead low while still removing stream startup work from the hotkey path.

### Recording State

`is_recording` should continue to represent the user-visible recording state. The new `armed` state should represent whether the persistent callback is currently allowed to buffer samples.

That separation keeps the UI and hotkey behavior unchanged:

- Press: clear stale samples, arm capture, mark recording active.
- Release: disarm capture, mark recording inactive, continue with the existing transcription path.

### Stream Recovery

If the persistent stream fails during initial setup, the app should keep the current failure behavior and surface the existing microphone error to the user.

If the stream later fails at runtime because of a device change or driver issue, the backend should:

1. Log the error.
2. Mark the stream as unavailable.
3. Refuse recording requests until a rebuild attempt is made.
4. Attempt to rebuild the stream on the next `start_recording` request.

This keeps the normal hotkey path fast while still allowing recovery after microphone changes.

### UX Implications

The microphone privacy indicator may remain active for the entire time FamVoice is running because the input stream stays open. This is an explicit trade-off accepted for lower press-to-capture latency.

CPU and battery impact should remain low because idle callbacks do minimal work, but the app will incur a small constant cost from holding the audio stream open.

## Testing

- Add unit tests for the new recording state transitions and buffer lifecycle.
- Verify that starting a recording clears stale samples before arming capture.
- Verify that idle callbacks do not append to the recording buffer.
- Verify that stopping a recording returns only samples captured while armed.
- Manually confirm that push-to-talk feels immediate and that the microphone indicator remains on while the app is running.

## Acceptance Criteria

- Hotkey press no longer performs per-recording microphone stream creation.
- Recording begins on the next active audio callback after the press path flips the armed state.
- Release still ends capture and triggers the existing transcription flow.
- Idle mode does not continue to accumulate audio samples.
- Runtime audio failures do not require restarting the app to recover.
