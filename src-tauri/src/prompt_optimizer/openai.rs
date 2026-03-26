use serde::{Deserialize, Serialize};

use super::metaprompt::SYSTEM_INSTRUCTION;
use super::{PromptOptimizerError, PromptOptimizerRequest};

pub const RESPONSES_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub const MAX_TOKENS: u32 = 1024;
pub const PROMPT_CACHE_KEY: &str = "famvoice-prompt-optimizer-v1";
pub const PROMPT_CACHE_RETENTION: &str = "in_memory";
pub const SUPPORTED_MODELS: [&str; 2] = ["gpt-5.4-mini", "gpt-5.4-nano"];

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ResponsesRequest {
    pub model: String,
    pub instructions: String,
    pub input: String,
    pub max_output_tokens: u32,
    pub prompt_cache_key: String,
    pub prompt_cache_retention: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ResponsesResponse {
    #[serde(default)]
    pub output: Vec<OutputItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct OutputItem {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub content: Vec<OutputContent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct OutputContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

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
) -> Result<ResponsesRequest, PromptOptimizerError> {
    validate_model(&request.model)?;

    Ok(ResponsesRequest {
        model: request.model.clone(),
        instructions: SYSTEM_INSTRUCTION.to_string(),
        input: request.source_transcript.clone(),
        max_output_tokens: MAX_TOKENS,
        prompt_cache_key: PROMPT_CACHE_KEY.to_string(),
        prompt_cache_retention: PROMPT_CACHE_RETENTION.to_string(),
    })
}

pub fn extract_optimized_prompt_text(
    response: ResponsesResponse,
) -> Result<String, PromptOptimizerError> {
    let text_blocks = response
        .output
        .into_iter()
        .filter(|item| item.item_type == "message")
        .flat_map(|item| item.content.into_iter())
        .filter(|content| content.content_type == "output_text")
        .filter_map(|content| content.text)
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
        let result = validate_model("claude-haiku-4-5");
        assert!(matches!(
            result,
            Err(PromptOptimizerError::UnsupportedModel { .. })
        ));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsupported model"));
    }

    #[test]
    fn request_payload_includes_chosen_model_and_source_transcript() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "uh write me a concise release note".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(&payload).unwrap();

        assert_eq!(payload_json["model"], SUPPORTED_MODELS[0]);
        assert_eq!(payload_json["instructions"], SYSTEM_INSTRUCTION);
        assert_eq!(payload_json["input"], "uh write me a concise release note");
        assert_eq!(payload_json["max_output_tokens"], MAX_TOKENS);
        assert_eq!(
            payload_json["prompt_cache_retention"],
            serde_json::Value::String("in_memory".to_string())
        );
        assert_eq!(
            payload_json["prompt_cache_key"],
            serde_json::Value::String("famvoice-prompt-optimizer-v1".to_string())
        );
    }

    #[test]
    fn request_payload_uses_metaprompt_system_instruction() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "test".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(&payload).unwrap();
        assert_eq!(payload_json["instructions"], SYSTEM_INSTRUCTION);
    }

    #[test]
    fn request_payload_does_not_include_reasoning_config_by_default() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "test".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(&payload).unwrap();

        assert!(payload_json.get("reasoning").is_none());
    }

    #[test]
    fn request_payload_sends_raw_transcript_without_preamble() {
        let request = PromptOptimizerRequest {
            model: SUPPORTED_MODELS[0].to_string(),
            source_transcript: "build me a todo app".to_string(),
        };

        let payload = build_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(&payload).unwrap();
        let user_text = payload_json["input"].as_str().unwrap();

        // Should be raw transcript, not wrapped in "Optimize the following..."
        assert_eq!(user_text, "build me a todo app");
    }

    #[test]
    fn blank_or_whitespace_only_model_output_is_rejected() {
        let response = serde_json::from_value::<ResponsesResponse>(serde_json::json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "   \n\t "
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let result = extract_optimized_prompt_text(response);
        assert!(matches!(
            result,
            Err(PromptOptimizerError::EmptyOptimizedPrompt)
        ));
    }

    #[test]
    fn only_the_final_text_block_is_used_for_the_optimized_prompt() {
        let response = serde_json::from_value::<ResponsesResponse>(serde_json::json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Objective:\nWrite a release note"
                        },
                        {
                            "type": "refusal",
                            "refusal": "not used"
                        },
                        {
                            "type": "output_text",
                            "text": "Constraints:\n- Keep it concise"
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let result = extract_optimized_prompt_text(response);
        assert_eq!(result.unwrap(), "Constraints:\n- Keep it concise");
    }
}
