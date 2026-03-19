# Persistent Mic Stream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove hotkey-press microphone startup delay by keeping one input stream alive and only arming buffering while the user is actively recording.

**Architecture:** The Rust audio thread will own one long-lived CPAL input stream plus an `armed` flag shared with the callback. `start_recording` will clear the buffer and arm capture instead of rebuilding the stream each time, while `stop_recording` will disarm capture and return the buffered samples without dropping the stream.

**Tech Stack:** Rust, Tauri, CPAL, Tokio

---

### Task 1: Add testable recording lifecycle helpers

**Files:**
- Modify: `src-tauri/src/audio.rs`

- [ ] **Step 1: Write the failing tests**

Add focused tests for:
- starting a recording clears stale buffered samples and arms capture
- stopping a recording disarms capture and returns only the armed samples
- idle callbacks still drop audio before processing

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recording_cycle_`
Expected: FAIL because the lifecycle helpers do not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Add small helper functions/structs for recording lifecycle state and buffer handoff.

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml recording_cycle_`
Expected: PASS

### Task 2: Keep the microphone stream alive across recordings

**Files:**
- Modify: `src-tauri/src/audio.rs`

- [ ] **Step 1: Refactor stream creation into a reusable builder**

Extract stream creation into a helper that can be used at startup and on rebuild after runtime failure.

- [ ] **Step 2: Replace per-recording stream creation with arm/disarm logic**

Build and `play()` the stream once when the audio thread starts, use an `armed` flag in the callback, and skip all processing while idle.

- [ ] **Step 3: Add rebuild-on-start fallback**

If the persistent stream is unavailable or marked broken, attempt to rebuild it on the next `start_recording`.

- [ ] **Step 4: Run the audio tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml audio::`
Expected: PASS

### Task 3: Verify the backend behavior

**Files:**
- Modify: `src-tauri/src/audio.rs`
- Review: `docs/superpowers/specs/2026-03-18-persistent-mic-stream-design.md`
- Review: `docs/superpowers/plans/2026-03-18-persistent-mic-stream.md`

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Review the working tree for intended audio changes**

Run: `git diff -- src-tauri/src/audio.rs docs/superpowers/specs/2026-03-18-persistent-mic-stream-design.md docs/superpowers/plans/2026-03-18-persistent-mic-stream.md`
Expected: Only the persistent-mic-stream work is present in those files.
