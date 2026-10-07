//! OpenRouter transcription transport; capture remains owned by FamVoice.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

use super::{
    scaled_request_timeout, user_facing_api_error, user_facing_network_error,
    validate_transcript_text, TranscriptionRequest, MAX_API_RESPONSE_BYTES,
};

const ENDPOINT: &str = "https://openrouter.ai/api/v1/audio/transcriptions";
const MAX_AUDIO_BYTES: usize = 32 * 1024 * 1024;
const MAX_SCRIBE_AUDIO_BYTES: usize = 25 * 1024 * 1024;

fn request_body(audio: &[u8], request: &TranscriptionRequest<'_>) -> Result<Value, String> {
    if !crate::settings::OPENROUTER_MODELS.contains(&request.model) {
        return Err("Unsupported OpenRouter transcription model.".into());
    }
    let max_audio_bytes = if request.model == crate::settings::OPENROUTER_SCRIBE_MODEL {
        MAX_SCRIBE_AUDIO_BYTES
    } else {
        MAX_AUDIO_BYTES
    };
    if audio.is_empty() || audio.len() > max_audio_bytes {
        return Err("OpenRouter recording is empty or exceeds the size limit.".into());
    }
    let format = match request.mime_type {
        "audio/flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => return Err("Unsupported OpenRouter recording format.".into()),
    };
    // Only literal vocabulary targets, never replacement values or instructions.
    let phrases: Vec<String> = request
        .keywords
        .iter()
        .filter(|phrase| {
            !phrase
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>'))
        })
        .map(|phrase| phrase.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|phrase| !phrase.is_empty() && phrase.chars().count() <= 64)
        .take(20)
        .collect();
    let mut body = json!({
        "model": request.model,
        "input_audio": { "data": STANDARD.encode(audio), "format": format },
        "response_format": "json"
    });
    body["provider"] = if request.model == crate::settings::OPENROUTER_SCRIBE_MODEL {
        let keyterms: Vec<String> = phrases
            .into_iter()
            .filter(|phrase| {
                phrase.chars().count() < 50
                    && phrase.split_whitespace().count() <= 5
                    && !phrase
                        .chars()
                        .any(|c| matches!(c, '{' | '}' | '[' | ']' | '\\'))
            })
            .collect();
        let mut options = json!({
            "diarize": false,
            "tag_audio_events": false,
            "timestamps_granularity": "none",
            "no_verbatim": false
        });
        // Only request paid vocabulary guidance when the user has valid terms.
        if !keyterms.is_empty() {
            options["keyterms"] = json!(keyterms);
        }
        json!({ "options": { "elevenlabs": options } })
    } else {
        json!({ "options": { "azure": {
            "enhancedMode": { "enabled": true,
                "modelOptions": { "transcribeStyle": "verbatim", "timestamps": "none" } },
            "diarization": { "enabled": false },
            "profanityFilterMode": "None",
            "phraseList": { "phrases": phrases }
        } } })
    };
    if let Some(language) = request
        .language
        .map(str::trim)
        .filter(|language| !language.is_empty() && *language != "auto")
    {
        if !crate::settings::SUPPORTED_LANGUAGE_PREFERENCES.contains(&language) {
            return Err("Unsupported OpenRouter transcription language.".into());
        }
        body["language"] = json!(language);
    }
    // request.prompt is deliberately ignored: dictation never rewrites a prompt.
    Ok(body)
}

fn response_text(bytes: &[u8]) -> Result<String, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "Invalid OpenRouter transcription response.".to_string())?;
    if !value["error"].is_null() {
        return Err(
            "OpenRouter could not complete the transcription. Check your account and try again."
                .into(),
        );
    }
    validate_transcript_text(
        value["text"]
            .as_str()
            .ok_or("Missing OpenRouter transcript.")?,
    )
}

pub(super) async fn transcribe(
    client: &reqwest::Client,
    audio: &[u8],
    api_key: &str,
    request: &TranscriptionRequest<'_>,
) -> Result<String, String> {
    transcribe_at(client, audio, api_key, request, ENDPOINT).await
}

async fn transcribe_at(
    client: &reqwest::Client,
    audio: &[u8],
    api_key: &str,
    request: &TranscriptionRequest<'_>,
    endpoint: &str,
) -> Result<String, String> {
    let body = request_body(audio, request)?;
    let mut response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .timeout(scaled_request_timeout(
            "openrouter",
            audio.len(),
            request.audio_duration,
        ))
        .json(&body)
        .send()
        .await
        .map_err(|error| user_facing_network_error(&error, "openrouter"))?;
    if !response.status().is_success() {
        // Do not read or expose provider error bodies, which may echo input.
        return Err(user_facing_api_error(response.status(), "openrouter"));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_API_RESPONSE_BYTES as u64)
    {
        return Err("OpenRouter response exceeded the size limit.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "OpenRouter response interrupted.".to_string())?
    {
        super::ensure_response_size(bytes.len(), chunk.len())?;
        bytes.extend_from_slice(&chunk);
    }
    response_text(&bytes)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{MockHttpServer, MockReply};
    use super::*;
    use std::time::Duration;

    fn request<'a>(mime_type: &'a str, keywords: &'a [String]) -> TranscriptionRequest<'a> {
        TranscriptionRequest {
            provider: "openrouter",
            model: crate::settings::OPENROUTER_MAI_MODEL,
            language: Some("pt"),
            prompt: Some("never-send-this-instruction"),
            keywords,
            mime_type,
            file_name: "dictation.wav",
            audio_duration: Duration::from_secs(2),
        }
    }

    #[tokio::test]
    async fn scribe_transport_uses_elevenlabs_options_and_the_existing_key() {
        for (mime, format) in [("audio/flac", "flac"), ("audio/wav", "wav")] {
            let server = MockHttpServer::start(vec![MockReply::Response {
                status: "200 OK",
                content_type: "application/json",
                chunks: vec![br#"{"text":"Abre o pull request","usage":{"cost":0.0001}}"#],
            }]);
            let hints = vec![
                "FamVoice".into(),
                "pull request".into(),
                "bad\nterm".into(),
                "[bad]".into(),
                "a".repeat(50),
                "one two three four five six".into(),
            ];
            let mut req = request(mime, &hints);
            req.model = crate::settings::OPENROUTER_SCRIBE_MODEL;
            let result = transcribe_at(
                &reqwest::Client::new(),
                &[1, 2, 3],
                "synthetic-existing-key",
                &req,
                &server.endpoint,
            )
            .await
            .unwrap();
            assert_eq!(result, "Abre o pull request");
            let captured = server.finish();
            assert_eq!(captured.len(), 1);
            let http = &captured[0];
            assert!(http
                .to_ascii_lowercase()
                .contains("authorization: bearer synthetic-existing-key"));
            let body: Value = serde_json::from_str(http.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], crate::settings::OPENROUTER_SCRIBE_MODEL);
            assert_eq!(body["input_audio"]["format"], format);
            assert_eq!(body["input_audio"]["data"], "AQID");
            assert_eq!(body["language"], "pt");
            assert!(body["provider"]["options"].get("azure").is_none());
            let options = &body["provider"]["options"]["elevenlabs"];
            assert_eq!(options["keyterms"], json!(["FamVoice", "pull request"]));
            assert_eq!(options["diarize"], false);
            assert_eq!(options["tag_audio_events"], false);
            assert_eq!(options["no_verbatim"], false);
            assert_eq!(options["timestamps_granularity"], "none");
            assert!(options.get("transcript_edit").is_none());
            assert!(body.get("prompt").is_none());
            assert!(!http.contains("never-send-this-instruction"));
        }
    }

    #[test]
    fn scribe_auto_detection_omits_language_and_unused_paid_guidance() {
        let mut req = request("audio/wav", &[]);
        req.model = crate::settings::OPENROUTER_SCRIBE_MODEL;
        req.language = Some("auto");
        let body = request_body(&[1], &req).unwrap();
        assert!(body.get("language").is_none());
        assert!(body["provider"]["options"]["elevenlabs"]
            .get("keyterms")
            .is_none());
        assert!(request_body(&vec![0; MAX_SCRIBE_AUDIO_BYTES + 1], &req).is_err());
    }

    #[tokio::test]
    async fn openrouter_transport_sends_base64_formats_verbatim_and_literal_hints() {
        for (mime, format) in [("audio/flac", "flac"), ("audio/wav", "wav")] {
            let server = MockHttpServer::start(vec![MockReply::Response {
                status: "200 OK",
                content_type: "application/json",
                chunks: vec![br#"{"text":"  Ol\u00e1 T3  "}"#],
            }]);
            let hints = vec!["FamVoice".into(), "bad\nterm".into()];
            let result = transcribe_at(
                &reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .unwrap(),
                &[1, 2, 3],
                "synthetic-key",
                &request(mime, &hints),
                &server.endpoint,
            )
            .await
            .unwrap();
            assert_eq!(result, "Olá T3");
            let captured = server.finish();
            let http = &captured[0];
            assert!(http
                .to_ascii_lowercase()
                .contains("authorization: bearer synthetic-key"));
            let body: Value = serde_json::from_str(http.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], "microsoft/mai-transcribe-2");
            assert_eq!(body["input_audio"]["format"], format);
            assert_eq!(body["input_audio"]["data"], "AQID");
            assert_eq!(body["language"], "pt");
            let options = &body["provider"]["options"]["azure"];
            assert_eq!(
                options["enhancedMode"]["modelOptions"]["transcribeStyle"],
                "verbatim"
            );
            assert_eq!(options["phraseList"]["phrases"], json!(["FamVoice"]));
            assert!(!http.contains("never-send-this-instruction"));
            assert!(body.get("prompt").is_none());
        }
    }

    #[tokio::test]
    async fn openrouter_transport_errors_are_sanitized_and_never_retried() {
        for (status, expected) in [
            ("401 Unauthorized", "authentication"),
            ("402 Payment Required", "balance"),
            ("429 Too Many Requests", "quota"),
            ("500 Internal Server Error", "unavailable"),
            ("307 Temporary Redirect", "307"),
        ] {
            let server = MockHttpServer::start(vec![MockReply::Response {
                status,
                content_type: "application/json",
                chunks: vec![b"secret-audio-sentinel"],
            }]);
            let error = transcribe_at(
                &reqwest::Client::new(),
                &[1],
                "synthetic-key",
                &request("audio/wav", &[]),
                &server.endpoint,
            )
            .await
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("secret-audio-sentinel"));
            assert!(!error.contains("synthetic-key"));
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[test]
    fn openrouter_response_rejects_errors_missing_empty_and_invalid_json() {
        for body in [
            b"invalid".as_slice(),
            br#"{"text":42}"#,
            br#"{}"#,
            br#"{"text":" \r\n "}"#,
            br#"{"text":"partial","error":{"message":"secret"}}"#,
        ] {
            let error = response_text(body).unwrap_err();
            assert!(!error.contains("secret"));
            assert!(!error.contains("partial"));
        }
    }

    #[tokio::test]
    async fn openrouter_transport_rejects_oversized_response() {
        let oversized: &'static [u8] =
            Box::leak(vec![b' '; MAX_API_RESPONSE_BYTES + 1].into_boxed_slice());
        let server = MockHttpServer::start(vec![MockReply::Response {
            status: "200 OK",
            content_type: "application/json",
            chunks: vec![oversized],
        }]);
        let error = transcribe_at(
            &reqwest::Client::new(),
            &[1],
            "synthetic-key",
            &request("audio/wav", &[]),
            &server.endpoint,
        )
        .await
        .unwrap_err();
        assert!(error.contains("size limit"));
        server.finish();
    }

    #[test]
    fn openrouter_request_enforces_audio_model_and_language_bounds() {
        let mut req = request("audio/wav", &[]);
        req.language = Some("auto");
        assert!(request_body(&[1], &req).unwrap().get("language").is_none());
        assert!(request_body(&[], &req).is_err());
        assert!(request_body(&vec![0; MAX_AUDIO_BYTES + 1], &req).is_err());
        req.model = "wrong-model";
        assert!(request_body(&[1], &req).is_err());
        req.model = crate::settings::OPENROUTER_MAI_MODEL;
        req.mime_type = "video/mp4";
        assert!(request_body(&[1], &req).is_err());
        req.mime_type = "audio/wav";
        req.language = Some("pt-PT");
        assert!(request_body(&[1], &req).is_err());
    }
}
