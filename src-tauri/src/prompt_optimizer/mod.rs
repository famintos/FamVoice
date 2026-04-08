mod metaprompt;
pub mod openai;

use std::fmt;

pub use openai::SUPPORTED_MODELS;

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
    UnusableOptimizedPrompt { reason: String },
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
                    openai::SUPPORTED_MODELS.join(", ")
                )
            }
            Self::MissingOptimizedPromptText => {
                write!(f, "response did not include an optimized prompt text block")
            }
            Self::EmptyOptimizedPrompt => write!(f, "optimized prompt text was blank"),
            Self::UnusableOptimizedPrompt { reason } => {
                write!(f, "optimized prompt text was unusable: {reason}")
            }
            Self::Http(error) => write!(f, "prompt optimizer request failed: {error}"),
            Self::InvalidResponse(error) => {
                write!(f, "invalid prompt optimizer response: {error}")
            }
        }
    }
}

impl std::error::Error for PromptOptimizerError {}

fn validate_optimized_prompt_text(optimized_prompt: &str) -> Result<(), PromptOptimizerError> {
    let normalized = optimized_prompt.trim().to_lowercase();
    let has_instruction_signal = [
        "inspect",
        "add ",
        "update",
        "fix ",
        "implement",
        "change",
        "preserve",
        "follow",
        "verify",
        "ensure",
        "remove",
        "keep ",
        "adjust",
        "rename",
        "wire ",
        "make ",
        "convert",
    ]
    .iter()
    .any(|signal| normalized.contains(signal));
    let has_section_signal = [
        "objective",
        "requested behavior",
        "implementation notes",
        "acceptance criteria",
        "testing and verification",
        "assumptions",
        "open questions",
    ]
    .iter()
    .any(|signal| normalized.contains(signal));
    let has_speech_style_marker = [
        "i want",
        "can you",
        "please",
        "quero",
        "quero que",
        "eu quero",
        "podes",
        "pode ",
        "podias",
        "preciso",
        "gostava",
    ]
    .iter()
    .any(|marker| normalized.contains(marker));

    if has_speech_style_marker {
        return Err(PromptOptimizerError::UnusableOptimizedPrompt {
            reason:
                "output still looks like conversational dictation instead of an execution prompt"
                    .to_string(),
        });
    }

    if !has_instruction_signal && !has_section_signal {
        return Err(PromptOptimizerError::UnusableOptimizedPrompt {
            reason: "output did not include recognizable execution guidance".to_string(),
        });
    }

    Ok(())
}

pub async fn optimize_prompt(
    client: &reqwest::Client,
    api_key: &str,
    request: PromptOptimizerRequest,
) -> Result<PromptOptimizerResponse, PromptOptimizerError> {
    let openai_request = openai::build_messages_request(&request)?;

    let response = client
        .post(openai::RESPONSES_ENDPOINT)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(std::time::Duration::from_secs(15))
        .json(&openai_request)
        .send()
        .await
        .map_err(|error| PromptOptimizerError::Http(error.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(PromptOptimizerError::Http(format!(
            "openai returned status {}: {}",
            status, body
        )));
    }

    let parsed = response
        .json::<openai::ResponsesResponse>()
        .await
        .map_err(|error| PromptOptimizerError::InvalidResponse(error.to_string()))?;

    let optimized_prompt = openai::extract_optimized_prompt_text(parsed)?;
    validate_optimized_prompt_text(&optimized_prompt)?;

    Ok(PromptOptimizerResponse { optimized_prompt })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_optimized_prompt_text_rejects_conversational_output() {
        let result = validate_optimized_prompt_text(
            "Podes adicionar aqui uma feature of M-Voice aqui à aplicação. Quero que as webs em vez de azul sejam amarelas",
        );

        assert!(matches!(
            result,
            Err(PromptOptimizerError::UnusableOptimizedPrompt { .. })
        ));
    }

    #[test]
    fn validate_optimized_prompt_text_accepts_execution_prompt() {
        let result = validate_optimized_prompt_text(
            "Inspect the existing voice UI implementation and change the wave color from blue to yellow while preserving the current structure and avoiding unrelated refactors.",
        );

        assert!(result.is_ok());
    }
}
