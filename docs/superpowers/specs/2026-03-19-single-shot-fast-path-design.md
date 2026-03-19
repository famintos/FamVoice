# Single-Shot Fast Path Design

## Goal

Reduce the delay between releasing the push-to-talk hotkey and receiving the final transcript, while preserving the current single-shot paste behavior.

## Product Context

FamVoice is currently a personal dictation tool. The priority for this change is lower release-to-text latency in daily use, not productization, collaboration, or public distribution.

The user primarily dictates in Portuguese but frequently mixes in English terms. The design therefore needs to improve speed without forcing a strict single-language mode that harms mixed-language accuracy.

## Scope

- Keep the current press-to-record, release-to-transcribe interaction model.
- Keep the current single-shot transcription model with one final result per recording.
- Reduce upload payload and post-release work by trimming obvious silence before encoding and upload.
- Improve perceived accuracy for mixed Portuguese and English dictation without adding a second transcription pass.
- Preserve the current clipboard, auto-paste, and history behavior.

## Non-goals

- Re-enabling live partial transcription while the user is still holding the hotkey.
- Showing draft transcripts in the UI.
- Adding a second-pass refinement model.
- Adding device selection, live meters, or noise suppression.
- Reworking window layout or the widget UI.

## Why This Path

The codebase has already invested in lower capture latency with a persistent microphone stream and background HTTP connection warmup. The next best place to win time is the release path in `stop_recording_cmd`, where the app still:

1. collects the full captured audio
2. encodes the whole clip to WAV
3. uploads the whole clip
4. waits for a final transcription response

That means silence at the beginning and end of a recording still increases both bytes sent and inference work after release.

The app also currently exposes rigid language choices. That can work against the real-world use case here, where Portuguese speech often contains English words, names, and technical terms.

## Design

### 1. Speech Windowing Before Upload

Before calling `encode_wav_in_memory`, the backend should analyze the recorded PCM and determine a safe speech window.

The speech windowing step should:

1. scan forward to find the first stable speech segment
2. scan backward to find the last stable speech segment
3. trim obvious leading and trailing silence
4. keep a small trailing buffer after the last speech frame so the final word is not clipped
5. return the original full clip if a safe trimmed window cannot be determined

This is intentionally conservative. The goal is not to build a sophisticated VAD system. The goal is to avoid sending clearly useless audio while preserving dictation reliability.

### 2. Conservative Windowing Heuristics

The implementation should use lightweight heuristics over short fixed-size frames derived from the existing 16 kHz mono sample stream.

Recommended first-pass behavior:

- frame size: `20-30ms`
- speech detection signal: RMS amplitude
- speech start rule: require multiple consecutive frames above a derived threshold
- speech end rule: require multiple consecutive frames below threshold
- trailing context keep-alive: `250-350ms`

These heuristics should be tuned for low false positives on trim. Missing a trim opportunity is acceptable. Clipping real speech is not.

### 3. Fast Path Integration

The new flow on release should be:

1. `stop_recording()` returns captured samples as it does today.
2. The backend checks for empty audio and existing silence rejection as it does today.
3. The backend optionally applies the existing quiet-audio gain logic.
4. The backend runs speech windowing on the adjusted samples.
5. If the windowed result is valid and shorter than the original clip, the backend encodes only that segment.
6. If windowing is uncertain, the backend falls back to the full sample buffer.
7. The app continues with the current single upload request and single final transcript response.

This keeps the architecture stable. The change lives entirely in the upload path and does not require reworking the hotkey or streaming model.

### 4. Fallback Rules

Fallback to the full clip should happen automatically when any of the following is true:

- no safe speech start is detected
- no safe speech end is detected
- the trimmed result would be suspiciously short
- the trim would remove too much of the clip relative to the detected energy pattern
- the clip is already short enough that trimming is unlikely to matter

The fallback should be silent and internal. The user should still receive the normal final transcript flow.

### 5. Mixed-Language Handling

The current language settings should be adjusted to better match the real dictation pattern.

The recommended UI model is:

- `Auto Detect`
- `Portuguese-first`
- `English-first`

For the first implementation, these choices should not hard-force the API request language to `pt` or `en` in the mixed-language case. Instead:

- `Auto Detect` continues to use no explicit language override
- `Portuguese-first` behaves like auto for the API request, but becomes the preferred product-facing default
- `English-first` behaves like auto for the API request, but is available for future heuristics and user intent

This avoids harming English words embedded in Portuguese dictation while still making the settings UI better reflect how the user actually speaks.

### 6. Personal Glossary Instead of Raw Replace

The current replacement system performs direct string replacement on the final transcript. That is brittle because it can rewrite substrings inside unrelated words.

The replacement system should evolve into a small personal glossary:

- intended for names, brands, English technical terms, and recurring phrases
- applied in a stable deterministic order
- aware of word boundaries where appropriate
- still simple enough to understand and edit manually in Settings

This improves perceived accuracy for the real `PT + EN terms` usage pattern without requiring model-side prompting or a second pass.

### 7. UI Changes

The fast path itself should not add a new toggle. It should be treated as an internal performance improvement and enabled by default.

The user-facing settings changes should be limited to:

- replacing the rigid language list with the simpler language preference options above
- relabeling replacements to make the glossary intent clearer if the final copy supports that

## Error Handling

- If speech windowing fails unexpectedly, the app should log the failure and continue with the original full clip.
- The existing "No voice detected" path should remain the gate for truly silent clips.
- Trimming should never be allowed to produce an empty upload.
- Glossary application should fail closed: if a glossary rule is malformed, skip it rather than corrupting the transcript.

## Testing

- Add unit tests for speech window detection on:
  - leading silence
  - trailing silence
  - both leading and trailing silence
  - very short clips
  - clips where trim confidence is low and fallback should keep the full buffer
- Add tests that ensure the trailing context buffer preserves the final word boundary.
- Add tests that verify mixed-language settings do not force a strict `pt` language override in the first implementation.
- Add tests that verify glossary replacements respect safer matching rules than raw substring replacement.
- Manually compare release-to-transcript latency before and after the change on normal dictations.

## Acceptance Criteria

- Release-to-transcript latency is perceptibly lower for normal dictations.
- The upload path no longer includes obvious leading and trailing silence when speech is clearly detected.
- Final words are not consistently clipped by the trim step.
- The app silently falls back to the full clip when speech windowing confidence is insufficient.
- Mixed Portuguese dictation with embedded English terms does not regress because of the language setting.
- Personal glossary corrections improve common repeated terms without creating obvious replacement artifacts.
