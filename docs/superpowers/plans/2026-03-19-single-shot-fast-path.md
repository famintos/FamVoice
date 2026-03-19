# Single-Shot Fast Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce release-to-transcript latency by trimming obvious silence before upload, while making mixed Portuguese and English dictation safer through language preference and glossary improvements.

**Architecture:** Keep the existing single-shot upload flow intact and insert a conservative speech-windowing step between local capture and WAV encoding. Preserve the current post-processing pipeline, but replace raw substring replacements with deterministic word-aware glossary application and simplify the language-setting model to match the intended mixed-language use case.

**Tech Stack:** Tauri v2, Rust, React, TypeScript, Vite

---

### Task 1: Backend speech windowing

**Files:**
- Modify: `src-tauri/src/audio.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add Rust unit tests in `src-tauri/src/audio.rs` for:
- clips with leading silence
- clips with trailing silence
- clips with both leading and trailing silence
- short clips that should keep the full buffer
- trailing context preservation

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test speech_window`
Expected: FAIL because the new trimming helpers do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add a focused helper in `src-tauri/src/audio.rs` that:
- analyzes 16 kHz mono PCM in short frames
- finds a conservative speech start and end
- keeps a small trailing context buffer
- returns either a trimmed clip or the original clip when confidence is low

Wire it into `stop_recording_cmd` in `src-tauri/src/lib.rs` after quiet-audio normalization and before `encode_wav_in_memory`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test speech_window`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/audio.rs src-tauri/src/lib.rs
git commit -m "feat: trim dictation audio before upload"
```

### Task 2: Safer glossary matching

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add Rust unit tests in `src-tauri/src/lib.rs` for:
- blank glossary targets being ignored
- whole-word replacements applying as expected
- substring replacements not mutating unrelated words
- multi-word phrase replacements staying deterministic

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test finalize_transcript`
Expected: FAIL on the new boundary-aware cases.

- [ ] **Step 3: Write minimal implementation**

Refactor transcript post-processing in `src-tauri/src/lib.rs` so glossary rules:
- are trimmed and filtered
- prefer longer targets first to avoid unstable overlap
- use case-insensitive word-boundary matching for simple single-word entries
- fall back to direct phrase replacement for entries containing whitespace or punctuation

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test finalize_transcript`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: improve transcript glossary matching"
```

### Task 3: Mixed-language setting refresh

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write the failing tests**

Add tests in `src-tauri/src/settings.rs` and `src-tauri/src/lib.rs` for:
- accepting `auto`, `pt-first`, and `en-first`
- loading legacy settings that still store `pt` and `en`
- keeping API language override unset for the new preference modes

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test language`
Expected: FAIL because the new options and normalization logic are not implemented yet.

- [ ] **Step 3: Write minimal implementation**

Update settings validation and normalization so:
- new saved values are `auto`, `pt-first`, and `en-first`
- legacy `pt` and `en` values are normalized safely on load
- transcription requests keep using `None` for the language override in all three current preference modes

Update `src/App.tsx` to show the new language preference labels and relabel replacements as a personal glossary.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test language`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/lib.rs src/App.tsx
git commit -m "feat: refresh language preferences for mixed dictation"
```

### Task 4: Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-19-single-shot-fast-path.md`

- [ ] **Step 1: Run targeted backend tests**

Run: `cargo test speech_window finalize_transcript language`
Expected: PASS

- [ ] **Step 2: Run app verification**

Run: `npm run build`
Expected: PASS

- [ ] **Step 3: Update plan checkboxes if needed and summarize verification**

Record any deviations, skipped checks, or follow-up risks in the final handoff.
