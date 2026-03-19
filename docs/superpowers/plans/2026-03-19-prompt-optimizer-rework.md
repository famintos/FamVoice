# Prompt Optimizer Rework Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the monolithic prompt optimizer with a modular architecture, a few-shot metaprompt system instruction, and no provider abstraction.

**Architecture:** Split `prompt_optimizer.rs` into `prompt_optimizer/` directory with three files (mod.rs, anthropic.rs, metaprompt.rs). Strip the `PromptOptimizerProvider` enum and all provider-related code. Replace the rigid single-paragraph system instruction with a few-shot metaprompt adapted for voice-to-prompt transformation.

**Tech Stack:** Rust (Tauri backend), TypeScript/React (frontend), reqwest (HTTP), serde (serialization)

**Spec:** `docs/superpowers/specs/2026-03-19-prompt-optimizer-rework-design.md`

---

### Task 1: Create metaprompt.rs — the new system instruction

**Files:**
- Create: `src-tauri/src/prompt_optimizer/metaprompt.rs`

- [ ] **Step 0: Create the prompt_optimizer directory**

```bash
mkdir -p src-tauri/src/prompt_optimizer
```

Note: The old `prompt_optimizer.rs` file coexists on disk with this new directory until Task 3 deletes it. Rust's module system won't conflict because we use a `#[path]` attribute for temporary test compilation.

- [ ] **Step 1: Write the failing test**

Create the metaprompt module file with only the test first. The test validates key elements of the system instruction.

```rust
// src-tauri/src/prompt_optimizer/metaprompt.rs

pub const SYSTEM_INSTRUCTION: &str = "";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_instruction_contains_few_shot_examples() {
        let instruction = SYSTEM_INSTRUCTION.to_lowercase();

        // Role definition
        assert!(
            instruction.contains("voice transcript"),
            "must define voice-to-prompt role"
        );

        // Few-shot examples present (check for example markers)
        assert!(
            instruction.contains("<example>") || instruction.contains("<transcript>"),
            "must include few-shot examples"
        );

        // Speech artifact handling
        assert!(
            instruction.contains("filler") || instruction.contains("um") || instruction.contains("uh"),
            "must address speech artifacts"
        );

        // Intent preservation
        assert!(
            instruction.contains("preserve") || instruction.contains("original intent"),
            "must instruct to preserve intent"
        );

        // Adaptive structure (not forced sections)
        assert!(
            instruction.contains("adapt") || instruction.contains("complexity"),
            "must support adaptive structure"
        );

        // No markdown/labels in output
        assert!(
            instruction.contains("no markdown") || instruction.contains("do not add markdown"),
            "must prohibit markdown wrapping"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test metaprompt::tests::system_instruction_contains_few_shot_examples -- --nocapture 2>&1`
Expected: FAIL — the empty string won't contain any of the required elements.

Note: The module won't compile yet since it's not declared anywhere. For this step, temporarily add to the bottom of `src-tauri/src/lib.rs`:

```rust
#[cfg(test)]
#[path = "prompt_optimizer/metaprompt.rs"]
mod metaprompt_test;
```

- [ ] **Step 3: Write the metaprompt system instruction**

Replace the empty `SYSTEM_INSTRUCTION` constant with the full metaprompt. Use Anthropic's metaprompt pattern adapted for voice-to-prompt transformation:

```rust
pub const SYSTEM_INSTRUCTION: &str = r#"You are a voice-to-prompt assistant. You receive raw voice transcripts — often messy, with filler words, false starts, and repetitions — and transform them into clear, effective prompts for AI assistants like Claude or ChatGPT.

<rules>
1. Clean speech artifacts: remove filler words (uh, um, like, you know, so, basically), false starts, repetitions, and verbal tics. Do not remove meaningful hedging or qualifications.
2. Preserve the user's original intent exactly. Do not invent requirements, add features, or embellish beyond what was said. If something is ambiguous, keep the ambiguity rather than guessing.
3. Adapt structure to complexity:
   - Simple requests → concise, direct prompt (1-3 sentences)
   - Medium requests → clear prompt with context and constraints
   - Complex multi-part requests → organized with sections (only add sections when they earn their weight)
4. Do not force a rigid template. Only add structure (headers, bullet points, numbered lists) when the request genuinely benefits from it.
5. Preserve the user's technical choices: if they name specific tools, languages, frameworks, or approaches, keep those exactly.
6. Return only the final prompt. Do not add markdown code fences, labels like "Prompt:", explanations, or surrounding quotation marks.
</rules>

<examples>
<example>
<transcript>uh so I need like a react component that shows a list of users and um you can filter them by name and also sort by like date joined or something and it should use tailwind for styling</transcript>
<prompt>Build a React component that displays a list of users with:
- A text filter that filters users by name
- Sort functionality by date joined
- Tailwind CSS for all styling</prompt>
</example>

<example>
<transcript>I want to write an email to my team about the deadline change so basically the project deadline moved from March 15th to April 1st and um we need to let everyone know that the scope hasn't changed just the timeline and uh make it professional but not too formal</transcript>
<prompt>Write a professional but approachable email to my team announcing that the project deadline has moved from March 15th to April 1st. Emphasize that only the timeline changed — the scope remains the same. Keep the tone warm and reassuring, not overly formal.</prompt>
</example>

<example>
<transcript>okay so I need to build an API endpoint that um takes in a CSV file upload and parses it and validates each row against a schema and then like stores the valid rows in postgres and returns a summary of what was imported and what failed with the specific errors for each failed row and uh it should handle large files so maybe stream the parsing and also I want rate limiting on this endpoint</transcript>
<prompt>Build an API endpoint for CSV file upload that:

1. Accepts a CSV file upload
2. Parses the CSV in a streaming fashion to handle large files
3. Validates each row against a defined schema
4. Stores valid rows in PostgreSQL
5. Returns a JSON summary containing:
   - Count and details of successfully imported rows
   - Count and details of failed rows, with the specific validation error for each
6. Includes rate limiting on the endpoint</prompt>
</example>
</examples>"#;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test metaprompt::tests::system_instruction_contains_few_shot_examples -- --nocapture 2>&1`
Expected: PASS

- [ ] **Step 5: Remove temporary test path from lib.rs**

Remove the `#[cfg(test)] #[path = ...] mod metaprompt_test;` block added in step 2. This was only needed to run the test before the full module structure exists.

- [ ] **Step 6: Commit**

Note: This file is intentionally not yet wired into the Rust module tree — `mod.rs` comes in Task 3.

```bash
git add src-tauri/src/prompt_optimizer/metaprompt.rs
git commit -m "feat: add few-shot metaprompt system instruction for voice-to-prompt"
```

---

### Task 2: Create anthropic.rs — Anthropic API layer

**Files:**
- Create: `src-tauri/src/prompt_optimizer/anthropic.rs`

- [ ] **Step 1: Write the failing tests**

Create `anthropic.rs` with types, constants, empty function stubs, and tests:

```rust
// src-tauri/src/prompt_optimizer/anthropic.rs

use serde::{Deserialize, Serialize};

use super::metaprompt::SYSTEM_INSTRUCTION;
use super::{PromptOptimizerError, PromptOptimizerRequest};

pub const MESSAGES_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
pub const MESSAGES_API_VERSION: &str = "2023-06-01";
pub const MAX_TOKENS: u32 = 1024;
pub const SUPPORTED_MODELS: [&str; 2] = ["claude-haiku-4-5", "claude-sonnet-4-6"];

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct MessagesResponse {
    pub content: Vec<ContentBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

pub fn validate_model(model: &str) -> Result<(), PromptOptimizerError> {
    todo!()
}

pub fn build_messages_request(
    request: &PromptOptimizerRequest,
) -> Result<MessagesRequest, PromptOptimizerError> {
    todo!()
}

pub fn extract_optimized_prompt_text(
    response: MessagesResponse,
) -> Result<String, PromptOptimizerError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_model_accepts_supported_models() {
        for model in SUPPORTED_MODELS {
            assert!(
                validate_model(model).is_ok(),
                "expected model {model} to be accepted"
            );
        }
    }

    #[test]
    fn validate_model_rejects_unknown_model() {
        let result = validate_model("gpt-4o");
        assert!(matches!(
            result,
            Err(PromptOptimizerError::UnsupportedModel { .. })
        ));
        assert!(result.unwrap_err().to_string().contains("unsupported model"));
    }

    #[test]
    fn request_payload_includes_chosen_model_and_source_transcript() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[1].to_string(),
            source_transcript: "uh write me a concise release note".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(&payload).unwrap();

        assert_eq!(payload_json["model"], SUPPORTED_MODELS[1]);
        assert_eq!(payload_json["messages"][0]["content"][0]["type"], "text");
        assert!(payload_json["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("uh write me a concise release note"));
    }

    #[test]
    fn request_payload_uses_metaprompt_system_instruction() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "test".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        assert_eq!(payload.system, SYSTEM_INSTRUCTION);
    }

    #[test]
    fn request_payload_sends_raw_transcript_without_preamble() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "build me a todo app".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let user_text = payload.messages[0].content[0].text.as_ref().unwrap();

        // Should be raw transcript, not wrapped in "Optimize the following..."
        assert_eq!(user_text, "build me a todo app");
    }

    #[test]
    fn blank_or_whitespace_only_model_output_is_rejected() {
        let response = MessagesResponse {
            content: vec![ContentBlock {
                block_type: "text".to_string(),
                text: Some("   \n\t ".to_string()),
            }],
        };

        let result = extract_optimized_prompt_text(response);
        assert!(matches!(
            result,
            Err(PromptOptimizerError::EmptyOptimizedPrompt)
        ));
    }

    #[test]
    fn only_the_final_text_block_is_used_for_the_optimized_prompt() {
        let response = serde_json::from_value::<MessagesResponse>(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": "Objective:\nWrite a release note"
                },
                {
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "scratchpad",
                    "input": {}
                },
                {
                    "type": "text",
                    "text": "Constraints:\n- Keep it concise"
                }
            ]
        }))
        .unwrap();

        let result = extract_optimized_prompt_text(response);
        assert_eq!(result.unwrap(), "Constraints:\n- Keep it concise");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

These won't compile yet because `mod.rs` doesn't exist. The deferred test run happens in **Task 3, Step 4** (`cargo test prompt_optimizer`), which will exercise all tests in both `anthropic.rs` and `metaprompt.rs`. For now, just verify the file is saved.

- [ ] **Step 3: Implement the functions**

Replace the three `todo!()` stubs:

```rust
pub fn validate_model(model: &str) -> Result<(), PromptOptimizerError> {
    if SUPPORTED_MODELS.contains(&model) {
        Ok(())
    } else {
        Err(PromptOptimizerError::UnsupportedModel {
            model: model.to_string(),
        })
    }
}

pub fn build_messages_request(
    request: &PromptOptimizerRequest,
) -> Result<MessagesRequest, PromptOptimizerError> {
    validate_model(&request.model)?;

    Ok(MessagesRequest {
        model: request.model.clone(),
        max_tokens: MAX_TOKENS,
        system: SYSTEM_INSTRUCTION.to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: vec![ContentBlock {
                block_type: "text".to_string(),
                text: Some(request.source_transcript.clone()),
            }],
        }],
    })
}

pub fn extract_optimized_prompt_text(
    response: MessagesResponse,
) -> Result<String, PromptOptimizerError> {
    let text_blocks = response
        .content
        .into_iter()
        .filter(|block| block.block_type == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>();

    if text_blocks.is_empty() {
        return Err(PromptOptimizerError::MissingOptimizedPromptText);
    }

    let non_empty_blocks = text_blocks
        .into_iter()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    if non_empty_blocks.is_empty() {
        Err(PromptOptimizerError::EmptyOptimizedPrompt)
    } else {
        non_empty_blocks
            .last()
            .cloned()
            .ok_or(PromptOptimizerError::EmptyOptimizedPrompt)
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/prompt_optimizer/anthropic.rs
git commit -m "feat: add anthropic API layer for prompt optimizer"
```

---

### Task 3: Create mod.rs — public API and wire up the module

**Files:**
- Create: `src-tauri/src/prompt_optimizer/mod.rs`
- Delete: `src-tauri/src/prompt_optimizer.rs`
- Modify: `src-tauri/src/lib.rs:8-9` (update module declaration)

- [ ] **Step 1: Create mod.rs with public types, re-exports, and optimize_prompt**

```rust
// src-tauri/src/prompt_optimizer/mod.rs

pub mod anthropic;
mod metaprompt;

use std::fmt;

pub use anthropic::SUPPORTED_MODELS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptOptimizerRequest {
    pub model: String,
    pub source_transcript: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptOptimizerResponse {
    pub optimized_prompt: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PromptOptimizerError {
    UnsupportedModel { model: String },
    MissingOptimizedPromptText,
    EmptyOptimizedPrompt,
    Http(String),
    InvalidResponse(String),
}

impl fmt::Display for PromptOptimizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedModel { model } => {
                write!(
                    f,
                    "unsupported model: {model}. supported models: {}",
                    anthropic::SUPPORTED_MODELS.join(", ")
                )
            }
            Self::MissingOptimizedPromptText => {
                write!(f, "response did not include an optimized prompt text block")
            }
            Self::EmptyOptimizedPrompt => write!(f, "optimized prompt text was blank"),
            Self::Http(error) => write!(f, "prompt optimizer request failed: {error}"),
            Self::InvalidResponse(error) => {
                write!(f, "invalid prompt optimizer response: {error}")
            }
        }
    }
}

impl std::error::Error for PromptOptimizerError {}

pub async fn optimize_prompt(
    client: &reqwest::Client,
    api_key: &str,
    request: PromptOptimizerRequest,
) -> Result<PromptOptimizerResponse, PromptOptimizerError> {
    let anthropic_request = anthropic::build_messages_request(&request)?;

    let response = client
        .post(anthropic::MESSAGES_ENDPOINT)
        .header("x-api-key", api_key)
        .header("anthropic-version", anthropic::MESSAGES_API_VERSION)
        .json(&anthropic_request)
        .send()
        .await
        .map_err(|error| PromptOptimizerError::Http(error.to_string()))?;

    if !response.status().is_success() {
        return Err(PromptOptimizerError::Http(format!(
            "anthropic returned status {}",
            response.status()
        )));
    }

    let parsed = response
        .json::<anthropic::MessagesResponse>()
        .await
        .map_err(|error| PromptOptimizerError::InvalidResponse(error.to_string()))?;

    let optimized_prompt = anthropic::extract_optimized_prompt_text(parsed)?;

    Ok(PromptOptimizerResponse { optimized_prompt })
}
```

- [ ] **Step 2: Delete the old prompt_optimizer.rs**

Use `git rm` so the deletion is staged for commit:

```bash
git rm src-tauri/src/prompt_optimizer.rs
```

This removes the old monolithic module. All its functionality is now split across `mod.rs`, `anthropic.rs`, and `metaprompt.rs`. Specifically:
- `OPTIMIZER_SYSTEM_INSTRUCTION` → renamed to `SYSTEM_INSTRUCTION` in `metaprompt.rs`
- `AnthropicMessagesRequest` / `AnthropicMessage` / `AnthropicContentBlock` / `AnthropicMessagesResponse` → renamed to `MessagesRequest` / `Message` / `ContentBlock` / `MessagesResponse` in `anthropic.rs` (prefix dropped since they're inside the `anthropic` module)
- `ANTHROPIC_MESSAGES_ENDPOINT` / `ANTHROPIC_MESSAGES_API_VERSION` / `OPTIMIZER_MAX_TOKENS` → renamed to `MESSAGES_ENDPOINT` / `MESSAGES_API_VERSION` / `MAX_TOKENS` in `anthropic.rs`
- `validate_prompt_optimizer_request()` → replaced by `validate_model()` in `anthropic.rs`
- `unsupported_provider_or_model_requests_return_a_clear_error` test → replaced by `validate_model_accepts_supported_models` and `validate_model_rejects_unknown_model` in `anthropic.rs`
- `system_instruction_enforces_structured_prompt_output` test → replaced by `system_instruction_contains_few_shot_examples` in `metaprompt.rs`

- [ ] **Step 3: Update lib.rs module declaration**

In `src-tauri/src/lib.rs`, change lines 8-9 from:

```rust
#[allow(dead_code)]
mod prompt_optimizer;
```

to:

```rust
mod prompt_optimizer;
```

- [ ] **Step 4: Run all prompt_optimizer tests**

Run: `cd src-tauri && cargo test prompt_optimizer -- --nocapture 2>&1`
Expected: All tests in `anthropic.rs` and `metaprompt.rs` PASS.

- [ ] **Step 5: Commit**

The old file was already staged for deletion via `git rm` in Step 2.

```bash
git add src-tauri/src/prompt_optimizer/mod.rs src-tauri/src/lib.rs
git commit -m "feat: restructure prompt optimizer into modular directory"
```

---

### Task 4: Update lib.rs — strip provider logic, simplify integration

**Files:**
- Modify: `src-tauri/src/lib.rs:561-608`

- [ ] **Step 1: Delete prompt_optimizer_provider() function**

Remove lines 561-570 in `lib.rs`:

```rust
// DELETE this entire function:
fn prompt_optimizer_provider(
    provider: &str,
) -> Option<prompt_optimizer::PromptOptimizerProvider> {
    match provider {
        prompt_optimizer::ANTHROPIC_PROVIDER => {
            Some(prompt_optimizer::PromptOptimizerProvider::Anthropic)
        }
        _ => None,
    }
}
```

- [ ] **Step 2: Simplify resolve_final_output_for_paste()**

Update the function body (signature stays the same). Remove the provider lookup guard block (lines 596-602) and simplify request construction. The function currently at line 572 becomes:

```rust
async fn resolve_final_output_for_paste<Optimize, OptimizeFuture>(
    settings: &AppSettings,
    finalized_transcript: String,
    timeout_duration: std::time::Duration,
    optimize: Optimize,
) -> String
where
    Optimize: FnOnce(prompt_optimizer::PromptOptimizerRequest) -> OptimizeFuture,
    OptimizeFuture: Future<
        Output = Result<
            prompt_optimizer::PromptOptimizerResponse,
            prompt_optimizer::PromptOptimizerError,
        >,
    >,
{
    if !settings.prompt_optimization_enabled {
        return finalized_transcript;
    }

    let api_key = settings.anthropic_api_key.trim();
    if api_key.is_empty() {
        return finalized_transcript;
    }

    let request = prompt_optimizer::PromptOptimizerRequest {
        model: settings.prompt_optimizer_model.clone(),
        source_transcript: finalized_transcript.clone(),
    };

    eprintln!(
        "{}",
        prompt_optimizer_start_message(&settings.prompt_optimizer_model)
    );
    let optimization_started_at = std::time::Instant::now();

    match tokio::time::timeout(timeout_duration, optimize(request)).await {
        Ok(Ok(response)) => {
            eprintln!(
                "{}",
                prompt_optimizer_success_message(
                    &settings.prompt_optimizer_model,
                    optimization_started_at.elapsed()
                )
            );
            response.optimized_prompt
        }
        Ok(Err(error)) => {
            eprintln!(
                "{}",
                prompt_optimizer_failure_message(
                    &settings.prompt_optimizer_model,
                    &error.to_string()
                )
            );
            finalized_transcript
        }
        Err(_) => {
            eprintln!(
                "{}",
                prompt_optimizer_timeout_message(
                    &settings.prompt_optimizer_model,
                    timeout_duration
                )
            );
            finalized_transcript
        }
    }
}
```

Key changes from current:
- Removed the `prompt_optimizer_provider()` guard block (lines 596-602)
- Request construction drops `provider` field

- [ ] **Step 3: Update lib.rs integration tests**

Update all `resolve_final_output_for_paste` tests to remove `prompt_optimizer_provider` from settings construction and remove the `request.provider` assertion. For each test that currently has:

```rust
prompt_optimizer_provider: "anthropic".to_string(),
```

Remove that line. And in `test_resolve_final_output_uses_optimized_output_on_success`, change the closure from:

```rust
|request| async move {
    assert_eq!(
        request.provider,
        prompt_optimizer::PromptOptimizerProvider::Anthropic
    );
    assert_eq!(request.model, "claude-haiku-4-5");
    assert_eq!(request.source_transcript, "final transcript");
    // ...
}
```

to:

```rust
|request| async move {
    assert_eq!(request.model, "claude-haiku-4-5");
    assert_eq!(request.source_transcript, "final transcript");
    // ...
}
```

Tests to update (remove `prompt_optimizer_provider` line from settings):
- `test_resolve_final_output_uses_optimized_output_on_success` (line 1264)
- `test_resolve_final_output_falls_back_when_optimizer_fails` (line 1296)
- `test_resolve_final_output_skips_optimizer_when_anthropic_key_is_blank` (line 1321)
- `test_resolve_final_output_falls_back_when_optimizer_times_out` (line 1352)

- [ ] **Step 4: Run lib.rs integration tests**

Run: `cd src-tauri && cargo test test_resolve_final_output -- --nocapture 2>&1`
Expected: All 5 tests PASS.

Run: `cd src-tauri && cargo test test_prompt_optimizer -- --nocapture 2>&1`
Expected: All 6 timeout/logging tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor: strip provider abstraction from prompt optimizer integration"
```

---

### Task 5: Update settings.rs — remove provider, rewire model validation

**Files:**
- Modify: `src-tauri/src/settings.rs:12-14,36-38,66-67,87,183-190,265,531,559,565-578`

- [ ] **Step 1: Remove provider constants, default function, and struct field**

In `settings.rs`:

1. Delete line 12: `pub const SUPPORTED_PROMPT_OPTIMIZER_PROVIDERS: [&str; 1] = ["anthropic"];`
2. Delete lines 13-14: `pub const SUPPORTED_PROMPT_OPTIMIZER_MODELS: [&str; 2] = ["claude-haiku-4-5", "claude-sonnet-4-6"];`
3. Delete lines 36-38: the `default_prompt_optimizer_provider()` function
4. Update `default_prompt_optimizer_model()` (line 40-42) to use the new module path:
   ```rust
   fn default_prompt_optimizer_model() -> String {
       crate::prompt_optimizer::SUPPORTED_MODELS[0].to_string()
   }
   ```
5. In `AppSettings` struct, remove lines 66-67:
   ```rust
   #[serde(default = "default_prompt_optimizer_provider")]
   pub prompt_optimizer_provider: String,
   ```
6. In `Default` impl, remove line 87:
   ```rust
   prompt_optimizer_provider: default_prompt_optimizer_provider(),
   ```

- [ ] **Step 2: Update validate_settings()**

Remove the provider validation block (lines 183-190):
```rust
// DELETE:
if !SUPPORTED_PROMPT_OPTIMIZER_PROVIDERS.contains(&settings.prompt_optimizer_provider.as_str())
{
    errors.push(format!(
        "Unsupported prompt optimizer provider: {}. Use one of: {}",
        settings.prompt_optimizer_provider,
        SUPPORTED_PROMPT_OPTIMIZER_PROVIDERS.join(", ")
    ));
}
```

Update the model validation block (lines 192-198) to use the new constant path:
```rust
if !crate::prompt_optimizer::SUPPORTED_MODELS.contains(&settings.prompt_optimizer_model.as_str()) {
    errors.push(format!(
        "Unsupported prompt optimizer model: {}. Use one of: {}",
        settings.prompt_optimizer_model,
        crate::prompt_optimizer::SUPPORTED_MODELS.join(", ")
    ));
}
```

- [ ] **Step 3: Update sample_settings() in tests**

Remove the `prompt_optimizer_provider` line from `sample_settings()`:
```rust
fn sample_settings() -> AppSettings {
    AppSettings {
        api_key: "sk-test".to_string(),
        model: "gpt-4o-mini-transcribe".to_string(),
        language: "auto".to_string(),
        auto_paste: true,
        preserve_clipboard: false,
        hotkey: "CommandOrControl+Shift+Space".to_string(),
        widget_mode: false,
        mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
        prompt_optimization_enabled: false,
        prompt_optimizer_model: "claude-haiku-4-5".to_string(),
        anthropic_api_key: String::new(),
        replacements: vec![],
    }
}
```

- [ ] **Step 4: Update and delete settings tests**

1. **Delete** `test_validate_settings_rejects_invalid_prompt_optimizer_provider` (lines 564-578) entirely.

2. **Update** `test_default_settings_include_prompt_optimizer_defaults` — remove line 531:
   ```rust
   // DELETE: assert_eq!(settings.prompt_optimizer_provider, "anthropic");
   ```

3. **Update** `test_settings_loads_prompt_optimizer_defaults_from_legacy_file` — remove line 559:
   ```rust
   // DELETE: assert_eq!(settings.prompt_optimizer_provider, "anthropic");
   ```

4. **Update** `test_settings_round_trip_persists_prompt_optimizer_settings` — remove lines 305, 316:
   ```rust
   // DELETE: settings.prompt_optimizer_provider = "anthropic".to_string();
   // DELETE: assert_eq!(settings.prompt_optimizer_provider, "anthropic");
   ```

5. **Add** new backward compat test:
   ```rust
   #[test]
   fn test_settings_deserializes_legacy_file_with_provider_field() {
       let dir = tempdir().unwrap();
       let path = dir.path().join("settings.json");
       fs::write(
           &path,
           r#"{
     "api_key": "sk-test",
     "model": "gpt-4o-mini-transcribe",
     "language": "en",
     "auto_paste": true,
     "preserve_clipboard": false,
     "hotkey": "CommandOrControl+Shift+Space",
     "widget_mode": false,
     "prompt_optimization_enabled": true,
     "prompt_optimizer_provider": "anthropic",
     "prompt_optimizer_model": "claude-sonnet-4-6",
     "anthropic_api_key": "sk-ant-old",
     "replacements": []
   }"#,
       )
       .unwrap();

       let state = SettingsState::load(dir.path().to_path_buf());
       let settings = state.settings.lock().unwrap();

       assert!(settings.prompt_optimization_enabled);
       assert_eq!(settings.prompt_optimizer_model, "claude-sonnet-4-6");
       assert_eq!(settings.anthropic_api_key, "sk-ant-old");
   }
   ```

- [ ] **Step 5: Run all settings tests**

Run: `cd src-tauri && cargo test settings::tests -- --nocapture 2>&1`
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "refactor: remove provider from settings, rewire model validation"
```

---

### Task 6: Update App.tsx — remove provider UI

**Files:**
- Modify: `src/App.tsx:44,79-81,300-311`

- [ ] **Step 1: Remove provider from TypeScript interface**

In `src/App.tsx`, remove line 44 from the `Settings` interface:
```typescript
// DELETE: prompt_optimizer_provider: string;
```

- [ ] **Step 2: Remove PROMPT_OPTIMIZER_PROVIDERS constant**

Delete lines 79-81:
```typescript
// DELETE:
const PROMPT_OPTIMIZER_PROVIDERS = [
  { value: "anthropic", label: "Anthropic" },
];
```

- [ ] **Step 3: Remove provider dropdown from JSX**

Delete lines 300-311 (the entire provider `<label>` block):
```tsx
{/* DELETE this entire label block: */}
<label className="text-xs text-gray-400 flex flex-col gap-1.5">
  Provider
  <select
    value={settings.prompt_optimizer_provider}
    onChange={e => setSettings({ ...settings, prompt_optimizer_provider: e.target.value })}
    className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
  >
    {PROMPT_OPTIMIZER_PROVIDERS.map(provider => (
      <option key={provider.value} value={provider.value}>{provider.label}</option>
    ))}
  </select>
</label>
```

- [ ] **Step 4: Verify the frontend builds**

Run: `cd C:/Users/henri/Desktop/app_test/FamVoice && npm run build 2>&1`
Expected: Build succeeds with no TypeScript errors.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "refactor: remove prompt optimizer provider dropdown from UI"
```

---

### Task 7: Full build and test verification

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests PASS. No compilation errors.

- [ ] **Step 2: Run full Tauri build**

Run: `cd C:/Users/henri/Desktop/app_test/FamVoice && npm run tauri build 2>&1`
Expected: Build succeeds.

- [ ] **Step 3: Verify no dead code warnings**

Check cargo output for any warnings about unused imports or dead code related to prompt_optimizer. There should be none.

- [ ] **Step 4: Commit (if any fixes were needed)**

Only commit if fixups were required during verification. Otherwise, skip.
