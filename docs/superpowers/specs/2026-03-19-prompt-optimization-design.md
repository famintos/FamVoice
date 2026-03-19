# Prompt Optimization Design

## Goal

Add an optional post-transcription step that rewrites dictated ideas into clearer, more structured, higher-quality prompts before they are pasted.

## Product Context

FamVoice is currently a dictation-first desktop app. The primary job is still to capture speech quickly and paste useful text into the active app with minimal friction.

The new feature should support a different use case: speaking a rough idea out loud, then receiving a prompt that is ready to use with coding agents and general-purpose LLM workflows.

The user wants this optimization step to follow current prompt-writing best practices from Anthropic and OpenAI, with Anthropic as the primary optimization provider in the first implementation.

## Scope

- Keep the current OpenAI transcription path as-is.
- Add a new optional prompt-optimization stage after transcript cleanup and glossary replacement.
- Make the optimizer architecture provider-agnostic even though v1 only wires Anthropic.
- Add settings for enabling the feature, configuring the provider/model, and storing the Anthropic API key.
- Paste and store the optimized prompt when optimization succeeds.
- Fall back to the normal transcript if optimization fails.

## Non-goals

- Replacing OpenAI transcription with Anthropic.
- Adding a local template-only optimizer in place of a model call.
- Building a prompt-template editor in v1.
- Showing a side-by-side preview chooser before pasting.
- Storing both the original transcript and optimized prompt in history in v1.
- Supporting multiple optimizer providers in the shipped v1 UI.

## Why This Path

The requested feature is meaningfully different from dictation cleanup. Glossary replacement can fix deterministic wording, but it cannot reliably:

1. infer the main objective from a rough spoken idea
2. separate context from requirements
3. surface constraints explicitly
4. define the desired output format
5. raise the quality bar of the final prompt

That requires a second model pass.

At the same time, hard-coding Anthropic-specific logic directly into the main transcription flow would make future changes harder than they need to be. The backend should instead introduce a small optimizer layer with a stable internal interface and a provider-specific implementation behind it.

## Design

### 1. End-to-End Flow

When prompt optimization is disabled, FamVoice should behave exactly as it does today.

When prompt optimization is enabled, the release flow becomes:

1. record audio
2. transcribe audio with the existing OpenAI path
3. run the existing transcript finalization and glossary logic
4. send the cleaned transcript to the prompt optimizer
5. receive the optimized prompt text
6. auto-paste and persist the optimized prompt instead of the raw transcript

The optimization step must operate on the finalized transcript, not raw audio output, so that glossary corrections still improve the optimizer input.

### 2. Provider-Agnostic Optimizer Layer

The backend should introduce a dedicated module such as `src-tauri/src/prompt_optimizer.rs`.

That module should define:

- a provider-neutral request shape
- a provider-neutral response shape
- a small trait or dispatch function for optimizer backends
- the Anthropic implementation for v1

Recommended internal request fields:

- source transcript text
- provider id
- model id
- optional optimization mode or quality hint

Recommended internal response fields:

- optimized text
- provider metadata needed for logging only

This keeps the `stop_recording_cmd` flow simple and prevents provider-specific request-building from spreading through `lib.rs`.

### 3. Settings Model

Add a new prompt-optimization group to persisted settings.

Recommended settings fields:

- `prompt_optimization_enabled: bool`
- `prompt_optimizer_provider: String`
- `prompt_optimizer_model: String`
- `anthropic_api_key: String`

v1 UI behavior:

- `Improve into prompt` checkbox
- `Provider` select, initially exposing only `Anthropic`
- `Model` select with `claude-haiku-4-5` and `claude-sonnet-4-6`
- `Anthropic API Key` password input

Defaults:

- optimization disabled
- provider default set to `anthropic`
- model default set to `claude-haiku-4-5`
- Anthropic API key empty by default

Older `settings.json` files must continue to load cleanly with these fields defaulted.

### 4. Optimizer Prompt Construction

The optimizer should not invent product requirements or expand the user's request beyond what was actually said. Its job is to reorganize and clarify the dictated idea into a prompt that is direct, structured, and immediately usable.

The Anthropic backend should use a fixed internal instruction set and pass the dictated transcript as content input. The model output should be the final prompt text only.

The output structure should generally aim to include:

- objective
- context
- constraints
- known inputs or facts
- desired output
- quality bar

The instruction set should enforce these rules:

- preserve intent
- make implicit constraints explicit when they are clearly implied by the user's wording
- avoid inventing facts
- avoid meta-commentary such as "here is your improved prompt"
- return concise, ready-to-use prompt text
- prefer explicit structure over prose blobs

This aligns with the user's goal of turning spoken rough ideas into higher-quality prompts using current best-practice prompt patterns rather than freeform paraphrasing.

### 5. Provider Selection Strategy

The first shipped provider should be Anthropic.

Recommended v1 model strategy:

- default model: `claude-haiku-4-5`
- high-quality option: `claude-sonnet-4-6`

Reasoning:

- `Haiku` keeps per-request cost low enough for frequent use.
- `Sonnet` remains available when the user wants a higher-quality rewrite of messy or underspecified ideas.
- the provider-agnostic backend still leaves room for a future OpenAI optimizer backend without restructuring the app again.

### 6. Failure and Fallback Rules

Prompt optimization must never break the core dictation workflow.

Fallback to the finalized transcript should happen when:

- prompt optimization is enabled but no Anthropic API key is configured
- the provider request fails
- the provider returns empty or invalid text
- the optimizer times out

Fallback behavior should be silent from a workflow perspective:

- paste the finalized transcript
- store the finalized transcript in history
- log the optimizer failure for debugging

The app should not surface a blocking error dialog for optimizer failure in v1.

### 7. History Behavior

In v1, history should store only the final text that was actually pasted.

That means:

- if optimization succeeds, history stores the optimized prompt
- if optimization falls back, history stores the finalized transcript

This avoids widening the history data model and UI before it is necessary.

### 8. UX Impact

This feature introduces a second remote model call, which increases release-to-paste latency.

That trade-off is acceptable because:

- the feature is optional
- its value comes from quality, not speed
- users who want plain dictation can leave it disabled

The UI should therefore keep the feature clearly framed as an optional enhancement rather than a default dictation behavior.

## Error Handling

- If optimizer settings are incomplete, skip optimization and continue with the normal transcript.
- If the Anthropic request fails unexpectedly, log the failure and continue with the normal transcript.
- If the optimizer returns blank output, treat it as a failed optimization and fall back.
- Sanitization should trim leading and trailing whitespace from the optimized result before use.
- The fallback path should not emit a separate user-visible error state in v1 unless the app's primary transcription flow also fails.

## Testing

- Add `settings.rs` tests for new optimizer defaults, validation, and backward compatibility with older settings files.
- Add prompt-optimizer unit tests for request construction, response parsing, blank-output rejection, and provider failure handling.
- Add `lib.rs` flow tests that verify:
  - optimization disabled preserves current behavior
  - optimization enabled uses optimized text on success
  - optimization enabled falls back to the finalized transcript on failure
- Manually test latency and paste behavior with:
  - optimization off
  - optimization on with Haiku
  - optimization on with Sonnet
  - optimization on with missing API key

## Acceptance Criteria

- Users can enable or disable prompt optimization from Settings.
- Users can configure an Anthropic API key and choose between `claude-haiku-4-5` and `claude-sonnet-4-6`.
- When optimization is off, FamVoice behaves exactly as it does today.
- When optimization is on and succeeds, the pasted and persisted text is the optimized prompt.
- When optimization is on and fails, the pasted and persisted text falls back to the finalized transcript without breaking the workflow.
- The optimizer backend is implemented behind a provider-agnostic internal interface rather than Anthropic-specific code spread through the app.
