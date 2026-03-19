use serde::{Deserialize, Serialize};
use std::fmt;

pub const ANTHROPIC_PROVIDER: &str = "anthropic";
pub const ANTHROPIC_MESSAGES_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_MESSAGES_API_VERSION: &str = "2023-06-01";
pub const ANTHROPIC_OPTIMIZER_MODELS: [&str; 2] = ["claude-haiku-4-5", "claude-sonnet-4-6"];
pub const OPTIMIZER_MAX_TOKENS: u32 = 1024;
pub const OPTIMIZER_SYSTEM_INSTRUCTION: &str = "You are a prompt optimization assistant. Produce a structured prompt with sections for objective, context, constraints, known inputs and facts, desired output, and quality bar. Preserve the original intent, avoid inventing facts, and prefer explicit structure over freeform prose. Return only the final optimized prompt as structured output. Do not add markdown, labels, explanations, or surrounding quotation marks.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptOptimizerProvider {
    Anthropic,
    OpenAI,
}

impl PromptOptimizerProvider {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => ANTHROPIC_PROVIDER,
            Self::OpenAI => "openai",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptOptimizerRequest {
    pub provider: PromptOptimizerProvider,
    pub model: String,
    pub source_transcript: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptOptimizerResponse {
    pub optimized_prompt: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PromptOptimizerError {
    UnsupportedProvider(String),
    UnsupportedModel { provider: String, model: String },
    MissingOptimizedPromptText,
    EmptyOptimizedPrompt,
    Http(String),
    InvalidResponse(String),
}

impl fmt::Display for PromptOptimizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider(provider) => {
                write!(f, "unsupported provider: {provider}")
            }
            Self::UnsupportedModel { provider, model } => {
                write!(
                    f,
                    "unsupported model for {provider}: {model}. supported models: {}",
                    ANTHROPIC_OPTIMIZER_MODELS.join(", ")
                )
            }
            Self::MissingOptimizedPromptText => {
                write!(f, "response did not include an optimized prompt text block")
            }
            Self::EmptyOptimizedPrompt => write!(f, "optimized prompt text was blank"),
            Self::Http(error) => write!(f, "prompt optimizer request failed: {error}"),
            Self::InvalidResponse(error) => write!(f, "invalid prompt optimizer response: {error}"),
        }
    }
}

impl std::error::Error for PromptOptimizerError {}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<AnthropicMessage>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AnthropicMessagesResponse {
    pub content: Vec<AnthropicContentBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

pub fn validate_prompt_optimizer_request(
    request: &PromptOptimizerRequest,
) -> Result<(), PromptOptimizerError> {
    match request.provider {
        PromptOptimizerProvider::Anthropic => {
            if ANTHROPIC_OPTIMIZER_MODELS.contains(&request.model.as_str()) {
                Ok(())
            } else {
                Err(PromptOptimizerError::UnsupportedModel {
                    provider: request.provider.as_str().to_string(),
                    model: request.model.clone(),
                })
            }
        }
        PromptOptimizerProvider::OpenAI => Err(PromptOptimizerError::UnsupportedProvider(
            request.provider.as_str().to_string(),
        )),
    }
}

pub fn build_anthropic_messages_request(
    request: &PromptOptimizerRequest,
) -> Result<AnthropicMessagesRequest, PromptOptimizerError> {
    validate_prompt_optimizer_request(request)?;

    Ok(AnthropicMessagesRequest {
        model: request.model.clone(),
        max_tokens: OPTIMIZER_MAX_TOKENS,
        system: OPTIMIZER_SYSTEM_INSTRUCTION.to_string(),
        messages: vec![AnthropicMessage {
            role: "user".to_string(),
            content: vec![AnthropicContentBlock {
                block_type: "text".to_string(),
                text: format!(
                    "Optimize the following source transcript into a polished prompt:\n\n{}",
                    request.source_transcript
                ),
            }],
        }],
    })
}

pub fn extract_optimized_prompt_text(
    response: AnthropicMessagesResponse,
) -> Result<String, PromptOptimizerError> {
    let block = response
        .content
        .last()
        .ok_or(PromptOptimizerError::MissingOptimizedPromptText)?;

    if block.block_type != "text" {
        return Err(PromptOptimizerError::MissingOptimizedPromptText);
    }

    let trimmed = block.text.trim();
    if trimmed.is_empty() {
        Err(PromptOptimizerError::EmptyOptimizedPrompt)
    } else {
        Ok(trimmed.to_string())
    }
}

pub async fn optimize_prompt(
    client: &reqwest::Client,
    api_key: &str,
    request: PromptOptimizerRequest,
) -> Result<PromptOptimizerResponse, PromptOptimizerError> {
    let anthropic_request = build_anthropic_messages_request(&request)?;

    let response = client
        .post(ANTHROPIC_MESSAGES_ENDPOINT)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_MESSAGES_API_VERSION)
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
        .json::<AnthropicMessagesResponse>()
        .await
        .map_err(|error| PromptOptimizerError::InvalidResponse(error.to_string()))?;

    let optimized_prompt = extract_optimized_prompt_text(parsed)?;

    Ok(PromptOptimizerResponse { optimized_prompt })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_request_payload_includes_chosen_model_and_source_transcript() {
        let request = PromptOptimizerRequest {
            provider: PromptOptimizerProvider::Anthropic,
            model: ANTHROPIC_OPTIMIZER_MODELS[1].to_string(),
            source_transcript: "uh write me a concise release note".to_string(),
        };

        let payload = build_anthropic_messages_request(&request).unwrap();
        let payload_json = serde_json::to_value(payload).unwrap();

        assert_eq!(payload_json["model"], ANTHROPIC_OPTIMIZER_MODELS[1]);
        assert_eq!(payload_json["messages"][0]["content"][0]["type"], "text");
        assert!(payload_json["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("uh write me a concise release note"));
    }

    #[test]
    fn system_instruction_enforces_structured_prompt_output() {
        let instruction = OPTIMIZER_SYSTEM_INSTRUCTION.to_lowercase();

        assert!(instruction.contains("structured prompt"));
        assert!(instruction.contains("objective"));
        assert!(instruction.contains("context"));
        assert!(instruction.contains("constraints"));
        assert!(instruction.contains("known inputs"));
        assert!(instruction.contains("desired output"));
        assert!(instruction.contains("quality bar"));
        assert!(instruction.contains("preserve the original intent"));
        assert!(instruction.contains("avoid inventing facts"));
        assert!(instruction.contains("explicit structure"));
    }

    #[test]
    fn blank_or_whitespace_only_model_output_is_rejected() {
        let response = AnthropicMessagesResponse {
            content: vec![AnthropicContentBlock {
                block_type: "text".to_string(),
                text: "   \n\t ".to_string(),
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
        let response = AnthropicMessagesResponse {
            content: vec![
                AnthropicContentBlock {
                    block_type: "text".to_string(),
                    text: "first draft".to_string(),
                },
                AnthropicContentBlock {
                    block_type: "tool_use".to_string(),
                    text: "ignored".to_string(),
                },
            ],
        };

        let result = extract_optimized_prompt_text(response);

        assert!(matches!(
            result,
            Err(PromptOptimizerError::MissingOptimizedPromptText)
        ));
    }

    #[test]
    fn unsupported_provider_or_model_requests_return_a_clear_error() {
        let provider_error = validate_prompt_optimizer_request(&PromptOptimizerRequest {
            provider: PromptOptimizerProvider::OpenAI,
            model: "gpt-4o".to_string(),
            source_transcript: "hello".to_string(),
        })
        .unwrap_err();

        assert!(provider_error.to_string().contains("unsupported provider"));

        let model_error = validate_prompt_optimizer_request(&PromptOptimizerRequest {
            provider: PromptOptimizerProvider::Anthropic,
            model: "bad-model".to_string(),
            source_transcript: "hello".to_string(),
        })
        .unwrap_err();

        assert!(model_error.to_string().contains("unsupported model"));
    }
}
