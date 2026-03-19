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
