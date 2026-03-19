# Prompt Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional post-transcription prompt optimizer that rewrites dictated ideas into structured prompts using Anthropic models behind a provider-agnostic backend interface.

**Architecture:** Keep the existing OpenAI transcription flow unchanged, then run a second model pass on the finalized transcript when prompt optimization is enabled. Persist the new optimizer settings in `settings.rs`, add a dedicated `prompt_optimizer.rs` backend module for provider dispatch and Anthropic integration, and update `stop_recording_cmd` to use the optimizer result with a safe fallback to the finalized transcript.

**Tech Stack:** Tauri v2, Rust, React 19, TypeScript, reqwest, serde, Anthropic Messages API

---

### Task 1: Persist Prompt Optimizer Settings

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Test: `src-tauri/src/settings.rs`

- [ ] **Step 1: Write the failing settings tests**

Add tests in `src-tauri/src/settings.rs` that verify:
- default settings disable prompt optimization
- default provider is `anthropic`
- default optimizer model is `claude-haiku-4-5`
- legacy settings files without optimizer fields still load successfully
- invalid provider or optimizer model is rejected

Expected new test names:
- `test_default_settings_include_prompt_optimizer_defaults`
- `test_settings_loads_prompt_optimizer_defaults_from_legacy_file`
- `test_validate_settings_rejects_invalid_prompt_optimizer_provider`
- `test_validate_settings_rejects_invalid_prompt_optimizer_model`

- [ ] **Step 2: Run the new settings tests to verify RED**

Run:
```bash
cargo test settings::tests::test_default_settings_include_prompt_optimizer_defaults
cargo test settings::tests::test_settings_loads_prompt_optimizer_defaults_from_legacy_file
cargo test settings::tests::test_validate_settings_rejects_invalid_prompt_optimizer_provider
cargo test settings::tests::test_validate_settings_rejects_invalid_prompt_optimizer_model
```

Expected: compile or assertion failures because the new settings fields and validation do not exist yet.

- [ ] **Step 3: Implement the settings model and validation**

In `src-tauri/src/settings.rs`:
- add persisted fields for `prompt_optimization_enabled`, `prompt_optimizer_provider`, `prompt_optimizer_model`, and `anthropic_api_key`
- add serde defaults so older `settings.json` files continue to deserialize cleanly
- add supported provider/model constants for the shipped v1 values
- extend `AppSettings::default()`
- extend `validate_settings()` with provider/model validation and a reasonable max length check for the Anthropic API key

- [ ] **Step 4: Run the settings tests to verify GREEN**

Run:
```bash
cargo test settings::tests
```

Expected: all settings tests pass, including the new optimizer coverage.

- [ ] **Step 5: Commit the settings task**

Run:
```bash
git add src-tauri/src/settings.rs
git commit -m "feat: add prompt optimizer settings"
```

### Task 2: Add Provider-Agnostic Prompt Optimizer Backend

**Files:**
- Create: `src-tauri/src/prompt_optimizer.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/prompt_optimizer.rs`

- [ ] **Step 1: Write the failing prompt optimizer tests**

Add tests in `src-tauri/src/prompt_optimizer.rs` for:
- Anthropic request payload includes the chosen model and source transcript
- the system instruction enforces structured prompt output
- blank or whitespace-only model output is rejected
- unsupported provider/model requests return a clear error

Expected new test names:
- `test_build_anthropic_request_includes_model_and_transcript`
- `test_optimizer_instruction_requires_direct_structured_output`
- `test_parse_optimizer_response_rejects_blank_output`
- `test_optimize_prompt_rejects_unsupported_provider`

- [ ] **Step 2: Run the new prompt optimizer tests to verify RED**

Run:
```bash
cargo test prompt_optimizer::tests
```

Expected: failures because the module and optimizer logic do not exist yet.

- [ ] **Step 3: Implement the optimizer module**

Create `src-tauri/src/prompt_optimizer.rs` with:
- provider-neutral request/response structs
- shipped provider/model constants for Anthropic
- a single public async entry point such as `optimize_prompt(client, request)`
- Anthropic Messages API request/response types
- a fixed optimizer system instruction aligned with the approved design
- response parsing that extracts only the final optimized prompt text and rejects blank output

Keep helper functions small and deterministic so they are easy to test without network calls.

- [ ] **Step 4: Wire the module into `lib.rs` without using it yet**

Add `mod prompt_optimizer;` and any imports needed for the integration task.

- [ ] **Step 5: Run the prompt optimizer tests to verify GREEN**

Run:
```bash
cargo test prompt_optimizer::tests
```

Expected: all prompt optimizer tests pass.

- [ ] **Step 6: Commit the optimizer backend task**

Run:
```bash
git add src-tauri/src/prompt_optimizer.rs src-tauri/src/lib.rs
git commit -m "feat: add Anthropic prompt optimizer backend"
```

### Task 3: Integrate Optimization Into Release-to-Paste Flow

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing flow tests**

Add tests in `src-tauri/src/lib.rs` around small extracted helpers so the integration remains testable without live API calls.

Cover:
- optimization disabled returns the finalized transcript unchanged
- optimization enabled uses optimized output on success
- optimization enabled falls back to the finalized transcript on optimizer failure
- optimization enabled skips optimizer calls when the Anthropic API key is blank

Expected new test names:
- `test_resolve_final_output_skips_optimizer_when_disabled`
- `test_resolve_final_output_uses_optimizer_text_on_success`
- `test_resolve_final_output_falls_back_to_transcript_on_optimizer_error`
- `test_resolve_final_output_skips_optimizer_when_anthropic_key_missing`

- [ ] **Step 2: Run the new flow tests to verify RED**

Run:
```bash
cargo test test_resolve_final_output_
```

Expected: failures because the helper and integration path do not exist yet.

- [ ] **Step 3: Implement the minimal integration**

In `src-tauri/src/lib.rs`:
- extract a small async helper that decides whether to call the optimizer and returns the final text to paste
- call that helper after `finalize_transcript()` and before clipboard/history work
- log optimizer failures and fall back to the finalized transcript
- wrap the optimizer call in a bounded timeout and treat timeout expiry as a normal fallback case
- preserve the existing success/error status behavior for the main transcription flow

Do not widen the history schema in v1. Persist only the final text actually pasted.

- [ ] **Step 4: Run the focused flow tests to verify GREEN**

Run:
```bash
cargo test test_resolve_final_output_
```

Expected: all new flow tests pass.

- [ ] **Step 5: Run the full Rust test suite**

Run:
```bash
cargo test
```

Expected: full backend suite passes with the optimizer integration in place.

- [ ] **Step 6: Commit the integration task**

Run:
```bash
git add src-tauri/src/lib.rs
git commit -m "feat: optimize prompts before paste"
```

### Task 4: Add Settings UI for Prompt Optimization

**Files:**
- Modify: `src/App.tsx`
- Test: build verification via `npm run build`

- [ ] **Step 1: Add the new TypeScript settings shape**

Update the `Settings` interface in `src/App.tsx` with the new optimizer fields added in Rust so the frontend can read and save them safely.

- [ ] **Step 2: Add the failing UI wiring by referencing the new fields**

Update the settings view to render a new `Prompt Optimization` section with:
- `Improve into prompt` checkbox
- `Provider` select
- `Model` select
- `Anthropic API Key` input

Intentionally build once before finishing the implementation so any missing fields or typing mismatches fail immediately.

- [ ] **Step 3: Run the frontend build to verify RED if wiring is incomplete**

Run:
```bash
npm run build
```

Expected: type or build errors until all new settings fields are correctly wired.

- [ ] **Step 4: Complete the UI implementation**

In `src/App.tsx`:
- add the new settings controls using the existing styling patterns
- disable or simplify provider choices to the shipped Anthropic option
- keep copy concise and explicit about the extra model pass
- ensure saving uses the existing `save_settings` flow without new commands

- [ ] **Step 5: Run the frontend build to verify GREEN**

Run:
```bash
npm run build
```

Expected: production build succeeds with the new settings UI.

- [ ] **Step 6: Commit the UI task**

Run:
```bash
git add src/App.tsx
git commit -m "feat: add prompt optimization settings UI"
```

### Task 5: Final Verification and Branch Readiness

**Files:**
- Verify: `src-tauri/src/settings.rs`
- Verify: `src-tauri/src/prompt_optimizer.rs`
- Verify: `src-tauri/src/lib.rs`
- Verify: `src/App.tsx`

- [ ] **Step 1: Run full verification**

Run:
```bash
cargo test
npm run build
```

Expected: both commands succeed on the implementation branch.

- [ ] **Step 2: Review the diff for spec alignment**

Check that the branch contains:
- provider-agnostic optimizer backend
- Anthropic settings and model selection
- fallback to finalized transcript on failure
- no history schema expansion beyond final pasted text

- [ ] **Step 3: Record final branch status**

Run:
```bash
git status --short
```
