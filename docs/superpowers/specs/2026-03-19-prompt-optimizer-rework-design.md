# Prompt Optimizer Rework

## Problem

The current prompt optimizer has two structural issues:

1. **Rigid, low-quality system instruction** — A single hardcoded paragraph forces every transcript into a fixed 6-section template (objective, context, constraints, known inputs, desired output, quality bar) regardless of task complexity. No few-shot examples, no speech artifact handling, no adaptive structure.

2. **Premature provider abstraction** — A `PromptOptimizerProvider` enum with an `OpenAI` variant that immediately errors. A provider dropdown locked to one option. Validation logic for a provider system that only supports one provider. Complexity without payoff.

## Decisions

These were explored and confirmed during brainstorming:

- **Always optimize when enabled** — no intent detection ("is this a prompt or a note?"). When the toggle is on, optimize. When off, raw transcription. Simple mental model.
- **Metaprompt approach** — replace the rigid instruction with Anthropic's proven few-shot metaprompt pattern, adapted for voice-to-prompt transformation.
- **Strip provider abstraction** — delete the provider enum, provider setting, provider validation, and provider UI. Build directly for Anthropic. Add other providers if/when needed.
- **Split into focused modules** — replace monolithic `prompt_optimizer.rs` with a `prompt_optimizer/` directory.

## Architecture

### Module Structure

```
src-tauri/src/prompt_optimizer/
├── mod.rs            # Public API: optimize_prompt(), Request, Response, Error types
├── anthropic.rs      # Anthropic API types, HTTP call, response parsing
└── metaprompt.rs     # System instruction constant
```

#### mod.rs — Public API

Exports the same interface that `lib.rs` currently consumes, but simplified:

```rust
pub struct PromptOptimizerRequest {
    pub model: String,
    pub source_transcript: String,
}

pub struct PromptOptimizerResponse {
    pub optimized_prompt: String,
}

pub enum PromptOptimizerError {
    UnsupportedModel { model: String },
    MissingOptimizedPromptText,
    EmptyOptimizedPrompt,
    Http(String),
    InvalidResponse(String),
}

pub async fn optimize_prompt(
    client: &reqwest::Client,
    api_key: &str,
    request: PromptOptimizerRequest,
) -> Result<PromptOptimizerResponse, PromptOptimizerError>;
```

Changes from current:
- `PromptOptimizerProvider` enum deleted
- `ANTHROPIC_PROVIDER` constant deleted (no longer needed)
- `UnsupportedProvider` error variant deleted
- `PromptOptimizerRequest.provider` field deleted
- `UnsupportedModel` no longer has a `provider` field — `Display` impl updated to drop "for {provider}" and just say "unsupported model: {model}"
- `validate_prompt_optimizer_request()` deleted and replaced by `validate_model()` in `anthropic.rs` (model-only check)

#### anthropic.rs — Anthropic API Layer

Contains all Anthropic-specific types and logic, moved from the current `prompt_optimizer.rs`:

- `AnthropicMessagesRequest`, `AnthropicMessage`, `AnthropicContentBlock` structs
- `AnthropicMessagesResponse` struct
- `build_anthropic_messages_request()` — constructs the API payload using the metaprompt
- `extract_optimized_prompt_text()` — parses the response for the final text block
- Constants: `ANTHROPIC_MESSAGES_ENDPOINT`, `ANTHROPIC_MESSAGES_API_VERSION`, `OPTIMIZER_MAX_TOKENS`
- Model validation: `SUPPORTED_MODELS` array and `validate_model()`

#### metaprompt.rs — System Instruction

A single file containing the `SYSTEM_INSTRUCTION` constant (replaces the current `OPTIMIZER_SYSTEM_INSTRUCTION`). Isolated for easy iteration.

### Metaprompt Design

The system instruction follows Anthropic's metaprompt pattern, adapted for FamVoice's voice-to-prompt use case.

**Structure:**
1. Role definition — "You receive raw voice transcripts and transform them into effective prompts for AI assistants"
2. Core rules:
   - Clean speech artifacts (filler words, false starts, repetitions)
   - Preserve the user's intent — do not invent requirements or embellish
   - Adapt structure to task complexity (simple tasks get concise prompts, complex tasks get more sections)
   - Return only the final prompt, no markdown wrapping, no labels, no explanations
3. Few-shot examples (2-3 before/after pairs):
   - **Coding task**: messy dictation of a React component request → clean, well-structured prompt
   - **Writing/creative task**: dictated email request → clean prompt with clear goal and constraints
   - **Complex multi-part task**: rambling multi-step request → organized prompt with clear sections
4. Anti-patterns — what NOT to do:
   - Don't force sections that aren't needed
   - Don't add boilerplate headers to simple requests
   - Don't change the user's technical choices or tool preferences

**User message format:** The user message simplifies to just the raw transcript — no wrapping preamble. The system instruction's few-shot examples already establish the expected input format, so the user message is simply the transcript text. This replaces the current `"Optimize the following source transcript into a polished prompt:\n\n{transcript}"` wrapper.

**Token budget:** ~800-1200 tokens for the system instruction. At Haiku pricing ($0.80/M input), this adds ~$0.001 per call. At Sonnet pricing ($3/M input), ~$0.004 per call.

### Settings Changes

**AppSettings (settings.rs):**

Remove:
- `prompt_optimizer_provider: String` field
- `SUPPORTED_PROMPT_OPTIMIZER_PROVIDERS` constant
- `SUPPORTED_PROMPT_OPTIMIZER_MODELS` constant (moves to `prompt_optimizer::anthropic::SUPPORTED_MODELS`)
- Provider validation in `validate_settings()`

Keep unchanged:
- `prompt_optimization_enabled: bool` (default: false)
- `prompt_optimizer_model: String` (default: "claude-haiku-4-5")
- `anthropic_api_key: String` (default: "")

Model validation moves to `prompt_optimizer::anthropic::SUPPORTED_MODELS` (re-exported from `prompt_optimizer::mod.rs` for clean imports) and is still called from `validate_settings()`.

**Backward compatibility:** Old settings files containing `prompt_optimizer_provider` are silently ignored — serde's default deserialization behavior skips JSON keys that have no matching struct field (since `AppSettings` does not use `#[serde(deny_unknown_fields)]`).

### lib.rs Integration Changes

**Delete:**
- `prompt_optimizer_provider()` function (lines 561-570)
- Remove `#[allow(dead_code)]` annotation on `mod prompt_optimizer` (the module is actively used)

**Simplify `resolve_final_output_for_paste()`:**
- Remove the `prompt_optimizer_provider()` lookup and its entire early-return guard block (lines 596-602)
- Construct `PromptOptimizerRequest` without a `provider` field

**Keep unchanged:**
- `prompt_optimizer_timeout()` — same 10s/30s per model
- `prompt_optimizer_timeout_message()`, `_start_message()`, `_success_message()`, `_failure_message()`
- Timeout-wrapped fallback logic
- `finalize_transcript()` — still runs before optimization

### Frontend Changes (App.tsx)

**Remove:**
- Provider dropdown and its state/handler

**Keep unchanged:**
- "Improve into prompt" checkbox
- Model dropdown (Haiku / Sonnet)
- Anthropic API key input

## Test Plan

### Tests to Delete
- `unsupported_provider_or_model_requests_return_a_clear_error` — provider half
- `test_validate_settings_rejects_invalid_prompt_optimizer_provider`
- Any test constructing `PromptOptimizerRequest` with a `provider` field (all updated)

### Tests to Update
- All tests constructing `PromptOptimizerRequest` — remove `provider` field
- `system_instruction_enforces_structured_prompt_output` — update assertions for new metaprompt content
- `test_default_settings_include_prompt_optimizer_defaults` — remove provider default assertion
- `test_settings_loads_prompt_optimizer_defaults_from_legacy_file` — remove provider assertion
- `test_settings_round_trip_persists_prompt_optimizer_settings` — remove provider from round-trip
- Integration tests in lib.rs — remove provider setup from request construction

### New Tests
- **Metaprompt content validation** — assert the system instruction contains: few-shot examples, speech artifact handling instructions, adaptive structure guidance, intent preservation rules
- **Model validation without provider** — assert `validate_model()` accepts supported models and rejects unknown ones
- **Backward compat** — assert old settings JSON containing `prompt_optimizer_provider` deserializes without error

### Tests Unchanged
- `blank_or_whitespace_only_model_output_is_rejected`
- `only_the_final_text_block_is_used_for_the_optimized_prompt`
- All timeout/logging tests in lib.rs
- All `resolve_final_output_for_paste` flow tests (after request construction update)
- All `finalize_transcript` tests

## What Stays the Same

- Enable toggle, model selection (Haiku/Sonnet), API key input
- Timeout behavior: 10s Haiku, 30s Sonnet
- Silent fallback to finalized transcript on any failure
- Logging format to stderr
- 1024 max tokens
- History stores final pasted text only
- Post-finalization optimization (glossary runs first)
