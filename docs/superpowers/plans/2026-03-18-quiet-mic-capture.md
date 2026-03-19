# Quiet Mic Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make FamVoice transcribe lower-volume speech more reliably by replacing the fixed silence cutoff with bounded auto-gain and a user-adjustable mic sensitivity setting.

**Architecture:** The Rust backend will compute clip loudness, derive a silence threshold from settings, and normalize quiet speech before WAV encoding. The React settings panel will expose one slider backed by the persisted Rust settings model so users can tune sensitivity without changing the rest of the flow.

**Tech Stack:** Rust, Tauri, React, TypeScript

---

### Task 1: Define Mic Sensitivity in Settings

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write a failing Rust test for default and validation behavior**

Add tests that expect `AppSettings::default()` to include a default `mic_sensitivity` and `validate_settings` to reject values outside the supported range.

- [ ] **Step 2: Run the Rust settings tests to verify failure**

Run: `cargo test settings:: --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because `mic_sensitivity` does not exist yet.

- [ ] **Step 3: Implement the settings model change**

Add `mic_sensitivity` to `AppSettings`, provide a default, and validate its allowed range.

- [ ] **Step 4: Update the React settings type and UI**

Add a slider and helper text in the Settings view so the new field can be edited and saved.

- [ ] **Step 5: Re-run the targeted tests**

Run: `cargo test settings:: --manifest-path src-tauri/Cargo.toml`
Expected: PASS

### Task 2: Replace the Fixed Silence Gate With Adjustable Quiet-Audio Processing

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing Rust tests for sensitivity thresholds and bounded normalization**

Add focused unit tests for helper functions that:
- classify clearly silent audio as silence
- keep quiet speech above the gate when sensitivity is increased
- normalize quiet clips without exceeding the configured gain cap

- [ ] **Step 2: Run the targeted lib tests to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mic_`
Expected: FAIL because the helper functions do not exist yet.

- [ ] **Step 3: Implement the helper functions and wire them into `stop_recording_cmd`**

Extract loudness processing into small testable helpers, remove the hardcoded `50.0` threshold, and apply bounded gain before WAV encoding when the clip is quiet but not silent.

- [ ] **Step 4: Re-run the targeted lib tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mic_`
Expected: PASS

### Task 3: Verify the End-to-End Build

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/App.tsx`

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Run the frontend build**

Run: `npm run build`
Expected: PASS

- [ ] **Step 3: Review the working tree**

Run: `git diff -- src-tauri/src/settings.rs src-tauri/src/lib.rs src/App.tsx docs/superpowers/specs/2026-03-18-quiet-mic-capture-design.md docs/superpowers/plans/2026-03-18-quiet-mic-capture.md`
Expected: Only the intended quiet-mic-capture changes are present.
