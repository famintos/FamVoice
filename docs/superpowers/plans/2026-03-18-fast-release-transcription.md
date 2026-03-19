# Fast Release Transcription Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the delay between hotkey release and final text insertion by streaming transcription work during recording and finalizing on release.

**Architecture:** The Rust backend will keep the existing local audio buffer, add a Realtime-oriented transcription session state that accepts streamed PCM chunks during recording, and finalize that session on release. If the streaming path is unavailable or fails, the existing `/v1/audio/transcriptions` flow remains the fallback so release reliability is preserved.

**Tech Stack:** Rust, Tauri, Tokio, Reqwest, OpenAI transcription APIs

---

### Task 1: Isolate release-time transcript selection logic

**Files:**
- Modify: `src-tauri/src/transcription.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write a failing Rust test for finalized transcript preference**

Add a small testable helper that prefers a finalized streaming transcript over fallback upload input and returns fallback when the streaming result is unavailable.

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml streaming_result_`
Expected: FAIL because the helper does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Add the selection helper and wire `stop_recording_cmd` through it without changing behavior yet.

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml streaming_result_`
Expected: PASS

### Task 2: Add chunked recording events for streaming consumers

**Files:**
- Modify: `src-tauri/src/audio.rs`

- [ ] **Step 1: Write failing Rust tests for chunk forwarding state**

Add focused tests for a small helper that decides whether captured PCM should be appended to the fallback buffer, forwarded to the streaming sink, or both.

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml audio_chunk_`
Expected: FAIL because the helper does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Introduce a chunk callback/sender path in `AudioState` so active recordings can forward PCM slices while still buffering locally for fallback.

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml audio_chunk_`
Expected: PASS

### Task 3: Add streaming transcription session state with safe fallback

**Files:**
- Modify: `src-tauri/src/transcription.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing Rust tests for session state transitions**

Add tests for a session-state helper that covers idle, active, degraded-to-fallback, and finalized states.

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml realtime_session_`
Expected: FAIL because the state helper does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Add `RealtimeTranscriptionState`, connect it to recording start/stop, and keep the current upload path as fallback.

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml realtime_session_`
Expected: PASS

### Task 4: Integrate finalization on release

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/transcription.rs`

- [ ] **Step 1: Write a failing Rust test for release-time finalize fallback behavior**

Add a helper-level test that verifies release uses the finalized streaming transcript when present and the legacy upload path when the streaming path times out or fails.

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml release_finalize_`
Expected: FAIL because the release helper does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Finalize the streaming session on stop, wait for the final transcript, and fall back to legacy upload when needed.

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml release_finalize_`
Expected: PASS

### Task 5: Verify the end-to-end build

**Files:**
- Modify: `src-tauri/src/audio.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/transcription.rs`
- Review: `docs/superpowers/specs/2026-03-18-fast-release-transcription-design.md`
- Review: `docs/superpowers/plans/2026-03-18-fast-release-transcription.md`

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Run the frontend build**

Run: `npm run build`
Expected: PASS

- [ ] **Step 3: Review the working tree for intended changes only**

Run: `git diff -- src-tauri/src/audio.rs src-tauri/src/lib.rs src-tauri/src/transcription.rs docs/superpowers/specs/2026-03-18-fast-release-transcription-design.md docs/superpowers/plans/2026-03-18-fast-release-transcription.md`
Expected: Only the fast-release transcription changes are present in those files.
