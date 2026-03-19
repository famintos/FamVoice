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
