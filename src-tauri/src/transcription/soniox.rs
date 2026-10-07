//! Soniox's WebSocket protocol, used for a completed in-memory dictation clip.
//! No remote file/job is created. Dropping the future closes the connection.

use super::{
    ensure_response_size, scaled_request_timeout, user_facing_api_error, validate_transcript_text,
    TranscriptionRequest, MAX_API_RESPONSE_BYTES,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{self, protocol::WebSocketConfig, Message},
};

pub(super) const ENDPOINT: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const AUDIO_CHUNK_BYTES: usize = 16 * 1024;
pub(super) const INVALID_RESPONSE: &str =
    "Soniox returned an invalid transcription response. Try again.";
pub(super) const INCOMPLETE_RESPONSE: &str =
    "Soniox closed the connection before completing the transcript. Try again.";
const TIMEOUT_ERROR: &str = "Soniox transcription timed out. Check the connection and try again.";

pub(super) fn configuration(api_key: &str, request: &TranscriptionRequest<'_>) -> Value {
    let mut config = json!({
        "api_key": api_key,
        "model": request.model,
        "audio_format": "auto",
        "context": { "terms": request.keywords },
    });
    if let Some(language) = request
        .language
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto")
    {
        // Portuguese dictation commonly includes English product/code terms.
        // Hints bias recognition; they do not translate or restrict the output.
        config["language_hints"] = if language == "pt" {
            json!(["pt", "en"])
        } else {
            json!([language])
        };
    }
    if let Some(prompt) = request.prompt.filter(|value| !value.trim().is_empty()) {
        config["context"]["text"] = json!(prompt);
    }
    config
}

#[derive(Deserialize)]
struct Token {
    text: String,
    is_final: bool,
    #[serde(default)]
    translation_status: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    tokens: Vec<Token>,
    #[serde(default)]
    finished: bool,
    #[serde(default)]
    error_code: Option<u16>,
    #[serde(default)]
    error_message: Option<String>,
}

#[derive(Default)]
pub(super) struct Transcript {
    text: String,
    received_bytes: usize,
}

impl Transcript {
    pub(super) fn push(&mut self, message: &str) -> Result<Option<String>, String> {
        ensure_response_size(self.received_bytes, message.len())?;
        self.received_bytes += message.len();
        // Never surface serde errors or provider bodies: they may contain audio
        // content, credentials, or arbitrary text supplied by the service.
        let response: Response =
            serde_json::from_str(message).map_err(|_| INVALID_RESPONSE.to_string())?;
        if let Some(code) = response.error_code {
            if code == 408 {
                // Classify documented timeout details into fixed local text.
                // Never return or log the provider's message itself.
                let stage = match response.error_message.as_deref() {
                    Some("Audio data decode timeout") => "decoding audio",
                    Some("Input too slow") => "receiving audio",
                    Some("Start request timeout") => "starting the request",
                    Some("Timed out while waiting for the first audio chunk") => {
                        "waiting for audio"
                    }
                    _ => "processing the request",
                };
                return Err(format!("Soniox timed out while {stage} (408). Try again."));
            }
            return Err(reqwest::StatusCode::from_u16(code)
                .map(|status| user_facing_api_error(status, "soniox"))
                .unwrap_or_else(|_| INVALID_RESPONSE.to_string()));
        }
        for token in response.tokens {
            if token.is_final
                && token.translation_status.as_deref() != Some("translation")
                && !matches!(token.text.as_str(), "<end>" | "<fin>")
            {
                ensure_response_size(self.text.len(), token.text.len())?;
                self.text.push_str(&token.text);
            }
        }
        if response.finished {
            return validate_transcript_text(&self.text).map(Some);
        }
        Ok(None)
    }
}

pub(super) fn connection_error(error: tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Http(response) => {
            reqwest::StatusCode::from_u16(response.status().as_u16())
                .map(|status| user_facing_api_error(status, "soniox"))
                .unwrap_or_else(|_| INVALID_RESPONSE.to_string())
        }
        tungstenite::Error::Capacity(_) => "Soniox response exceeded the size limit.".to_string(),
        _ => "Could not complete the Soniox connection. Check the connection and try again."
            .to_string(),
    }
}

pub(super) async fn transcribe(
    audio: &[u8],
    api_key: &str,
    request: &TranscriptionRequest<'_>,
) -> Result<String, String> {
    let timeout = scaled_request_timeout("soniox", audio.len(), request.audio_duration);
    transcribe_at(ENDPOINT, audio, api_key, request, timeout).await
}

async fn transcribe_at(
    endpoint: &str,
    audio: &[u8],
    api_key: &str,
    request: &TranscriptionRequest<'_>,
    timeout: Duration,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Soniox API key missing".to_string());
    }
    if audio.is_empty() {
        return Err("No audio to transcribe. Try speaking again.".to_string());
    }
    // Bound the entire operation, including handshake, upload and finalization.
    // Do not replay audio automatically after a failure (duplicate billing).
    tokio::time::timeout(timeout, async {
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_API_RESPONSE_BYTES))
            .max_frame_size(Some(MAX_API_RESPONSE_BYTES));
        let (mut socket, _) = connect_async_with_config(endpoint, Some(config), true)
            .await
            .map_err(connection_error)?;
        socket
            .send(Message::Text(
                configuration(api_key.trim(), request).to_string().into(),
            ))
            .await
            .map_err(connection_error)?;
        let (mut sender, mut receiver) = socket.split();
        let send_audio = async {
            for chunk in audio.chunks(AUDIO_CHUNK_BYTES) {
                sender
                    .send(Message::Binary(chunk.to_vec().into()))
                    .await
                    .map_err(connection_error)?;
            }
            // Use text EOF: live FLAC/WAV probes with empty binary EOF timed
            // out (408), while the same clips completed with empty text EOF.
            sender
                .send(Message::Text("".into()))
                .await
                .map_err(connection_error)
        };
        let receive_text = async {
            let mut transcript = Transcript::default();
            while let Some(message) = receiver.next().await {
                match message.map_err(connection_error)? {
                    Message::Text(text) => {
                        if let Some(text) = transcript.push(&text)? {
                            return Ok(text);
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => {}
                    _ => return Err(INVALID_RESPONSE.to_string()),
                }
            }
            // Never inject a partial transcript after a disconnected stream.
            Err(INCOMPLETE_RESPONSE.to_string())
        };
        let (_, text) = tokio::try_join!(send_audio, receive_text)?;
        Ok(text)
    })
    .await
    .map_err(|_| TIMEOUT_ERROR.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn request<'a>(language: Option<&'a str>, keywords: &'a [String]) -> TranscriptionRequest<'a> {
        TranscriptionRequest {
            model: "stt-rt-v5",
            language,
            prompt: Some("European Portuguese dictation"),
            keywords,
            provider: "soniox",
            mime_type: "audio/wav",
            file_name: "audio.wav",
            audio_duration: Duration::from_secs(1),
        }
    }

    #[test]
    fn hints_preserve_auto_and_support_portuguese_with_english() {
        let terms = vec!["FamVoice".to_string(), "pull request".to_string()];
        let config = configuration("synthetic-key", &request(Some("pt"), &terms));
        assert_eq!(config["language_hints"], json!(["pt", "en"]));
        assert_eq!(config["context"]["terms"], json!(terms));
        assert!(config.get("translation").is_none());
        assert!(config.get("language_hints_strict").is_none());
        for language in [None, Some("auto"), Some("")] {
            assert!(configuration("test", &request(language, &[]))
                .get("language_hints")
                .is_none());
        }
        assert_eq!(
            configuration("test", &request(Some("fr"), &[]))["language_hints"],
            json!(["fr"])
        );
    }

    #[test]
    fn only_final_original_tokens_are_delivered_after_finished() {
        let mut transcript = Transcript::default();
        assert!(transcript.push(r#"{"tokens":[{"text":"wrong provisional","is_final":false},{"text":"Olá","is_final":true}]}"#).unwrap().is_none());
        assert_eq!(transcript.push(r#"{"tokens":[{"text":" pull request\npronto.","is_final":true},{"text":"<end>","is_final":true},{"text":"<fin>","is_final":true},{"text":"translated","is_final":true,"translation_status":"translation"}],"finished":true}"#).unwrap(), Some("Olá pull request\npronto.".to_string()));
    }

    #[test]
    fn empty_invalid_and_oversized_responses_fail_without_remote_content() {
        let mut transcript = Transcript::default();
        assert!(transcript
            .push(r#"{"finished":true}"#)
            .unwrap_err()
            .contains("no text"));
        assert_eq!(
            transcript.push("private-sentinel").unwrap_err(),
            INVALID_RESPONSE
        );
        assert_eq!(
            transcript
                .push(r#"{"tokens":[{"text":"private-sentinel","is_final":"secret"}]}"#)
                .unwrap_err(),
            INVALID_RESPONSE
        );
        assert!(transcript
            .push(&"x".repeat(MAX_API_RESPONSE_BYTES + 1))
            .unwrap_err()
            .contains("size limit"));
        let mut bounded = Transcript {
            received_bytes: MAX_API_RESPONSE_BYTES,
            ..Transcript::default()
        };
        assert!(bounded.push("{}").unwrap_err().contains("size limit"));
    }

    #[test]
    fn provider_errors_never_expose_body_or_deliver_partial_text() {
        for (code, expected) in [
            (401, "authentication"),
            (402, "balance"),
            (429, "quota"),
            (503, "unavailable"),
        ] {
            let mut transcript = Transcript::default();
            transcript
                .push(r#"{"tokens":[{"text":"partial","is_final":true}]}"#)
                .unwrap();
            let message =
                json!({"error_code":code, "error_message":"private-sentinel", "finished":true});
            let error = transcript.push(&message.to_string()).unwrap_err();
            assert!(error.contains(expected));
            assert!(!error.contains("private-sentinel"));
            assert!(!error.contains("partial"));
        }
    }

    enum Reply {
        Messages(Vec<String>),
        Disconnect,
        WaitForDisconnect,
    }

    async fn server(
        reply: Reply,
    ) -> (
        String,
        tokio::task::JoinHandle<(Value, Vec<u8>)>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let config: Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            let mut audio = Vec::new();
            loop {
                match socket.next().await.unwrap().unwrap() {
                    Message::Text(text) if text.is_empty() => break,
                    Message::Binary(bytes) if bytes.is_empty() => {
                        // Observed live: binary EOF is ignored, then the request
                        // expires. Fail immediately here to keep regression fast.
                        socket
                            .send(Message::Text(
                                r#"{"error_code":408,"error_message":"Request timeout."}"#.into(),
                            ))
                            .await
                            .unwrap();
                        return (config, audio);
                    }
                    Message::Binary(bytes) => audio.extend_from_slice(&bytes),
                    _ => panic!("Expected audio frames"),
                }
            }
            let _ = ready_tx.send(());
            match reply {
                Reply::Messages(messages) => {
                    for message in messages {
                        socket.send(Message::Text(message.into())).await.unwrap();
                    }
                }
                Reply::Disconnect => {
                    socket.close(None).await.unwrap();
                }
                Reply::WaitForDisconnect => {
                    let result = tokio::time::timeout(Duration::from_secs(3), socket.next()).await;
                    assert!(matches!(
                        result,
                        Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_))))
                    ));
                }
            }
            (config, audio)
        });
        (endpoint, task, ready_rx)
    }

    #[tokio::test]
    async fn websocket_sends_text_eof_to_prevent_provider_408() {
        let (endpoint, server, _ready) = server(Reply::Messages(vec![
            r#"{"tokens":[{"text":"guess","is_final":false},{"text":"Olá ","is_final":true}]}"#
                .to_string(),
            r#"{"tokens":[{"text":"deploy","is_final":true}],"finished":true}"#.to_string(),
        ]))
        .await;
        let audio = vec![3u8; AUDIO_CHUNK_BYTES * 2 + 7];
        let text = transcribe_at(
            &endpoint,
            &audio,
            "synthetic-key",
            &request(Some("pt"), &[]),
            Duration::from_secs(3),
        )
        .await
        .unwrap();
        assert_eq!(text, "Olá deploy");
        let (config, received) = server.await.unwrap();
        assert_eq!(received, audio);
        assert_eq!(config["api_key"], "synthetic-key");
        assert_eq!(config["model"], "stt-rt-v5");
        assert_eq!(config["audio_format"], "auto");
        assert_eq!(config["language_hints"], json!(["pt", "en"]));
    }

    #[tokio::test]
    async fn provider_timeout_reports_only_a_safe_stage() {
        for (detail, stage) in [
            ("Audio data decode timeout", "decoding audio"),
            ("Input too slow", "receiving audio"),
            ("Start request timeout", "starting the request"),
            (
                "Timed out while waiting for the first audio chunk",
                "waiting for audio",
            ),
            ("private-sentinel", "processing the request"),
        ] {
            let (endpoint, server, _) = server(Reply::Messages(vec![
                json!({"error_code":408,"error_type":"request_timeout","error_message":detail})
                    .to_string(),
            ]))
            .await;
            let error = transcribe_at(
                &endpoint,
                &[1],
                "test",
                &request(None, &[]),
                Duration::from_secs(3),
            )
            .await
            .unwrap_err();
            assert!(error.contains(stage), "Missing safe timeout stage");
            assert!(error.contains("408"));
            assert!(!error.contains("private-sentinel"));
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn disconnected_stream_is_not_a_success() {
        let (endpoint, server, _ready) = server(Reply::Disconnect).await;
        let error = transcribe_at(
            &endpoint,
            &[1],
            "test",
            &request(None, &[]),
            Duration::from_secs(3),
        )
        .await
        .unwrap_err();
        assert_eq!(error, INCOMPLETE_RESPONSE);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn timeout_drops_socket_without_replaying_audio() {
        let (endpoint, server, _ready) = server(Reply::WaitForDisconnect).await;
        let error = transcribe_at(
            &endpoint,
            &[1],
            "test",
            &request(None, &[]),
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();
        assert_eq!(error, TIMEOUT_ERROR);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cancellation_drops_socket() {
        let (endpoint, server, ready) = server(Reply::WaitForDisconnect).await;
        let task = tokio::spawn(async move {
            transcribe_at(
                &endpoint,
                &[1],
                "test",
                &request(None, &[]),
                Duration::from_secs(3),
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        server.await.unwrap();
    }
}
