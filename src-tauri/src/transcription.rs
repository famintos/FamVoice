use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use reqwest::multipart;
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

const REALTIME_TRANSCRIPTION_MODEL: &str = "gpt-4o-mini-transcribe";
const REALTIME_HOST_MODEL: &str = "gpt-realtime";
const REALTIME_SAMPLE_RATE: usize = 24_000;
const REALTIME_COMMIT_INTERVAL_SAMPLES: usize = REALTIME_SAMPLE_RATE;
const REALTIME_FINALIZE_TIMEOUT: Duration = Duration::from_secs(4);
const REALTIME_INITIAL_EVENT_TIMEOUT: Duration = Duration::from_secs(2);

fn realtime_transcription_url() -> String {
    format!(
        "wss://api.openai.com/v1/realtime?model={}",
        REALTIME_HOST_MODEL
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseTranscriptDecision {
    StreamingFinal(String),
    FallbackUpload,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeSessionStatus {
    Inactive,
    Active,
    FallbackOnly,
    Finalized,
}

#[allow(dead_code)]
impl RealtimeSessionStatus {
    pub fn mark_failed(&mut self) {
        *self = Self::FallbackOnly;
    }

    pub fn mark_finalized(&mut self) {
        *self = Self::Finalized;
    }
}

pub fn select_release_transcript(
    finalized_transcript: Option<String>,
) -> ReleaseTranscriptDecision {
    match finalized_transcript {
        Some(text) => ReleaseTranscriptDecision::StreamingFinal(text),
        None => ReleaseTranscriptDecision::FallbackUpload,
    }
}

#[allow(dead_code)]
pub fn finalize_release_transcript<F>(
    finalized_transcript: Option<String>,
    fallback: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    match select_release_transcript(finalized_transcript) {
        ReleaseTranscriptDecision::StreamingFinal(text) => Ok(text),
        ReleaseTranscriptDecision::FallbackUpload => fallback(),
    }
}

pub struct RealtimeTranscriptionState {
    session: Mutex<Option<RealtimeTranscriptionSession>>,
}

impl Default for RealtimeTranscriptionState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

impl RealtimeTranscriptionState {
    pub fn replace(&self, session: Option<RealtimeTranscriptionSession>) {
        if let Ok(mut current) = self.session.lock() {
            *current = session;
        }
    }

    pub fn take(&self) -> Option<RealtimeTranscriptionSession> {
        self.session.lock().ok().and_then(|mut current| current.take())
    }
}

pub struct RealtimeTranscriptionSession {
    chunk_tx: mpsc::UnboundedSender<Vec<i16>>,
    finish_tx: Option<oneshot::Sender<()>>,
    result_rx: oneshot::Receiver<Result<Option<String>, String>>,
}

impl RealtimeTranscriptionSession {
    pub fn chunk_sender(&self) -> mpsc::UnboundedSender<Vec<i16>> {
        self.chunk_tx.clone()
    }

    pub async fn finish(mut self) -> Result<Option<String>, String> {
        if let Some(finish_tx) = self.finish_tx.take() {
            let _ = finish_tx.send(());
        }

        self.result_rx
            .await
            .map_err(|e| format!("Realtime transcription channel error: {e}"))?
    }
}

fn join_transcript_parts(parts: &[String]) -> Option<String> {
    let joined = parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    (!joined.is_empty()).then_some(joined)
}

fn pcm16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn upsample_16k_to_24k(samples: &[i16]) -> Vec<i16> {
    if samples.len() < 2 {
        return samples.to_vec();
    }

    let mut upsampled = Vec::with_capacity(samples.len() * 3 / 2 + 2);
    let mut position = 0.0f64;
    let step = 2.0 / 3.0;
    let last_index = (samples.len() - 1) as f64;

    while position < last_index {
        let index = position.floor() as usize;
        let fraction = position - index as f64;
        let left = samples[index] as f64;
        let right = samples[index + 1] as f64;
        let interpolated = left + (right - left) * fraction;
        upsampled.push(interpolated.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
        position += step;
    }

    if let Some(&last) = samples.last() {
        upsampled.push(last);
    }

    upsampled
}

async fn send_realtime_event<S>(
    writer: &mut S,
    event: serde_json::Value,
) -> Result<(), String>
where
    S: futures_util::sink::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    writer
        .send(Message::Text(event.to_string()))
        .await
        .map_err(|e| e.to_string())
}

async fn maybe_commit_audio_buffer<S>(
    writer: &mut S,
    buffered_since_commit: &mut usize,
    pending_commit: &mut bool,
) -> Result<(), String>
where
    S: futures_util::sink::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if *pending_commit || *buffered_since_commit == 0 {
        return Ok(());
    }

    send_realtime_event(writer, json!({ "type": "input_audio_buffer.commit" })).await?;
    *pending_commit = true;
    *buffered_since_commit = 0;
    Ok(())
}

fn build_realtime_session_update(language: Option<&str>) -> serde_json::Value {
    let mut transcription = serde_json::Map::new();
    transcription.insert(
        "model".to_string(),
        serde_json::Value::String(REALTIME_TRANSCRIPTION_MODEL.to_string()),
    );
    if let Some(language) = language {
        transcription.insert(
            "language".to_string(),
            serde_json::Value::String(language.to_string()),
        );
    }

    json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "audio": {
                "input": {
                    "format": {
                        "type": "audio/pcm",
                        "rate": REALTIME_SAMPLE_RATE,
                    },
                    "transcription": transcription,
                    "turn_detection": serde_json::Value::Null,
                }
            }
        }
    })
}

fn initial_session_type(event: &serde_json::Value) -> Option<&str> {
    (event.get("type").and_then(|value| value.as_str()) == Some("session.created"))
        .then(|| event.get("session").and_then(|value| value.get("type")).and_then(|value| value.as_str()))
        .flatten()
}

fn initial_server_error_message(event: &serde_json::Value) -> Option<String> {
    (event.get("type").and_then(|value| value.as_str()) == Some("error"))
        .then(|| {
            event.get("error")
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .unwrap_or("Realtime transcription error")
                .to_string()
        })
}

async fn wait_for_initial_server_event<S>(
    reader: &mut S,
) -> Result<Option<String>, String>
where
    S: futures_util::stream::Stream<
            Item = Result<Message, tokio_tungstenite::tungstenite::Error>,
        > + Unpin,
{
    let deadline = tokio::time::Instant::now() + REALTIME_INITIAL_EVENT_TIMEOUT;

    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = tokio::time::timeout(remaining, reader.next())
            .await
            .map_err(|_| "Timed out waiting for initial realtime server event".to_string())?;

        match message {
            Some(Ok(Message::Text(text))) => {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };

                if let Some(error) = initial_server_error_message(&event) {
                    return Err(error);
                }

                if let Some(session_type) = initial_session_type(&event) {
                    return Ok(Some(session_type.to_string()));
                }
            }
            Some(Ok(Message::Close(_))) => return Ok(None),
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(error.to_string()),
            None => return Ok(None),
        }
    }

    Ok(None)
}

pub async fn start_realtime_transcription_session(
    api_key: &str,
    language: Option<&str>,
) -> Result<RealtimeTranscriptionSession, String> {
    let mut request = realtime_transcription_url()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|e| e.to_string())?,
    );

    let (websocket, _) = connect_async(request).await.map_err(|e| e.to_string())?;
    let (mut writer, mut reader) = websocket.split();
    if let Some(session_type) = wait_for_initial_server_event(&mut reader).await? {
        eprintln!(
            "[FamVoice] Realtime server created initial session type: {}",
            session_type
        );
    }
    send_realtime_event(&mut writer, build_realtime_session_update(language)).await?;

    let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<Vec<i16>>();
    let (finish_tx, mut finish_rx) = oneshot::channel::<()>();
    let (result_tx, result_rx) = oneshot::channel::<Result<Option<String>, String>>();

    tokio::spawn(async move {
        let mut transcript_parts: Vec<String> = Vec::new();
        let mut buffered_since_commit = 0usize;
        let mut pending_commit = false;
        let mut finish_requested = false;
        let finish_sleep = tokio::time::sleep_until(
            tokio::time::Instant::now() + REALTIME_FINALIZE_TIMEOUT
        );
        tokio::pin!(finish_sleep);

        loop {
            tokio::select! {
                maybe_chunk = chunk_rx.recv(), if !finish_requested => {
                    match maybe_chunk {
                        Some(samples_16k) => {
                            let samples_24k = upsample_16k_to_24k(&samples_16k);
                            let payload = base64::engine::general_purpose::STANDARD
                                .encode(pcm16_to_bytes(&samples_24k));
                            if let Err(error) = send_realtime_event(
                                &mut writer,
                                json!({
                                    "type": "input_audio_buffer.append",
                                    "audio": payload,
                                }),
                            ).await {
                                let _ = result_tx.send(Err(error));
                                return;
                            }
                            buffered_since_commit += samples_24k.len();
                            if buffered_since_commit >= REALTIME_COMMIT_INTERVAL_SAMPLES {
                                if let Err(error) = maybe_commit_audio_buffer(
                                    &mut writer,
                                    &mut buffered_since_commit,
                                    &mut pending_commit,
                                ).await {
                                    let _ = result_tx.send(Err(error));
                                    return;
                                }
                            }
                        }
                        None => {
                            finish_requested = true;
                        }
                    }
                }
                _ = &mut finish_rx, if !finish_requested => {
                    finish_requested = true;
                    finish_sleep.as_mut().reset(tokio::time::Instant::now() + REALTIME_FINALIZE_TIMEOUT);
                    if let Err(error) = maybe_commit_audio_buffer(
                        &mut writer,
                        &mut buffered_since_commit,
                        &mut pending_commit,
                    ).await {
                        let _ = result_tx.send(Err(error));
                        return;
                    }

                    if !pending_commit {
                        let _ = result_tx.send(Ok(join_transcript_parts(&transcript_parts)));
                        return;
                    }
                }
                message = reader.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) else {
                                continue;
                            };

                            match event.get("type").and_then(|value| value.as_str()) {
                                Some("conversation.item.input_audio_transcription.completed") => {
                                    let transcript_text = event.get("transcript").and_then(|value| value.as_str()).unwrap_or("");
                                    eprintln!("[FamVoice] Realtime transcription chunk: {:?}", transcript_text);
                                    let transcript = transcript_text.trim();
                                    if !transcript.is_empty() {
                                        transcript_parts.push(transcript.to_string());
                                    }
                                    pending_commit = false;

                                    if finish_requested {
                                        if let Err(error) = maybe_commit_audio_buffer(
                                            &mut writer,
                                            &mut buffered_since_commit,
                                            &mut pending_commit,
                                        ).await {
                                            let _ = result_tx.send(Err(error));
                                            return;
                                        }

                                        if !pending_commit {
                                            let _ = result_tx.send(Ok(join_transcript_parts(&transcript_parts)));
                                            return;
                                        }
                                    } else if buffered_since_commit >= REALTIME_COMMIT_INTERVAL_SAMPLES {
                                        if let Err(error) = maybe_commit_audio_buffer(
                                            &mut writer,
                                            &mut buffered_since_commit,
                                            &mut pending_commit,
                                        ).await {
                                            let _ = result_tx.send(Err(error));
                                            return;
                                        }
                                    }
                                }
                                Some("error") => {
                                    let message = event
                                        .get("error")
                                        .and_then(|value| value.get("message"))
                                        .and_then(|value| value.as_str())
                                        .unwrap_or("Realtime transcription error")
                                        .to_string();
                                    eprintln!("[FamVoice] Realtime server error: {}", message);
                                    let _ = result_tx.send(Err(message));
                                    return;
                                }
                                Some("session.updated") => {
                                    eprintln!("[FamVoice] Realtime session configured");
                                }
                                Some(other) => {
                                    eprintln!("[FamVoice] Realtime event: {}", other);
                                }
                                None => {}
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            let _ = result_tx.send(Ok(join_transcript_parts(&transcript_parts)));
                            return;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(error)) => {
                            let _ = result_tx.send(Err(error.to_string()));
                            return;
                        }
                        None => {
                            let _ = result_tx.send(Ok(join_transcript_parts(&transcript_parts)));
                            return;
                        }
                    }
                }
                _ = &mut finish_sleep, if finish_requested => {
                    let _ = result_tx.send(Err("Realtime transcription timed out".to_string()));
                    return;
                }
            }
        }
    });

    Ok(RealtimeTranscriptionSession {
        chunk_tx,
        finish_tx: Some(finish_tx),
        result_rx,
    })
}

pub async fn transcribe_audio(
    client: &reqwest::Client,
    wav_bytes: Vec<u8>,
    api_key: &str,
    model: &str,
    language: Option<&str>,
) -> Result<String, String> {
    eprintln!("[FamVoice] Sending {} bytes to API", wav_bytes.len());

    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("response_format", "text");

    if let Some(lang) = language {
        if lang != "auto" {
            form = form.text("language", lang.to_string());
        }
    }

    let res = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(30))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let text = res.text().await.map_err(|e| e.to_string())?;
        Ok(text.trim().to_string())
    } else {
        let err_text = res.text().await.unwrap_or_default();
        Err(format!("API Error: {}", err_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_result_prefers_finalized_transcript_over_fallback_upload() {
        let decision = select_release_transcript(Some("ready now".to_string()));

        assert_eq!(
            decision,
            ReleaseTranscriptDecision::StreamingFinal("ready now".to_string())
        );
    }

    #[test]
    fn streaming_result_requests_fallback_when_no_finalized_transcript_exists() {
        let decision = select_release_transcript(None);

        assert_eq!(decision, ReleaseTranscriptDecision::FallbackUpload);
    }

    #[test]
    fn realtime_session_transitions_to_fallback_after_failure() {
        let mut session = RealtimeSessionStatus::Active;

        session.mark_failed();

        assert_eq!(session, RealtimeSessionStatus::FallbackOnly);
    }

    #[test]
    fn release_finalize_prefers_streaming_result_when_it_is_available() {
        let outcome = finalize_release_transcript(Some("done".to_string()), || {
            Ok("legacy".to_string())
        })
        .unwrap();

        assert_eq!(outcome, "done");
    }

    #[test]
    fn release_finalize_uses_fallback_when_streaming_result_is_missing() {
        let outcome = finalize_release_transcript(None, || Ok("legacy".to_string())).unwrap();

        assert_eq!(outcome, "legacy");
    }

    #[test]
    fn realtime_websocket_url_uses_ga_host_model_for_transcription_sessions() {
        let url = realtime_transcription_url();

        assert_eq!(url, "wss://api.openai.com/v1/realtime?model=gpt-realtime");
        assert!(!url.contains(REALTIME_TRANSCRIPTION_MODEL));
    }

    #[test]
    fn realtime_session_update_configures_transcription_in_audio_input() {
        let event = build_realtime_session_update(Some("pt"));

        assert_eq!(
            event["session"]["type"],
            serde_json::Value::String("realtime".to_string())
        );
        assert_eq!(
            event["session"]["audio"]["input"]["transcription"]["model"],
            serde_json::Value::String(REALTIME_TRANSCRIPTION_MODEL.to_string())
        );
        assert_eq!(
            event["session"]["audio"]["input"]["transcription"]["language"],
            serde_json::Value::String("pt".to_string())
        );
        assert_eq!(
            event["session"]["audio"]["input"]["turn_detection"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn initial_session_type_reads_session_created_event_type() {
        let event = serde_json::json!({
            "type": "session.created",
            "session": {
                "type": "realtime"
            }
        });

        assert_eq!(initial_session_type(&event), Some("realtime"));
    }

    #[test]
    fn initial_server_error_message_reads_error_event_message() {
        let event = serde_json::json!({
            "type": "error",
            "error": {
                "message": "boom"
            }
        });

        assert_eq!(initial_server_error_message(&event).as_deref(), Some("boom"));
    }
}
