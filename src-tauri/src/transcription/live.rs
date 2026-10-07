//! One bounded Soniox connection per recording. No partial delivery or automatic replay.
use super::{soniox, TranscriptionRequest, MAX_API_RESPONSE_BYTES};
use crate::{audio::AudioState, settings::AppSettings};
use futures_util::{SinkExt, StreamExt};
use std::{sync::Mutex, time::Duration};
use tokio::{sync::oneshot, task::JoinHandle};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};

const FINALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_TIMEOUT: Duration = Duration::from_secs(330);

#[derive(Default)]
pub(crate) struct LiveState(Mutex<Option<(u64, LiveSession)>>);

impl LiveState {
    pub fn start(&self, id: u64, audio: AudioState, settings: AppSettings) {
        let mut slot = self.0.lock().unwrap();
        *slot = Some((id, LiveSession::start(id, audio, settings)));
    }

    pub fn take(&self, id: u64) -> Option<LiveSession> {
        let mut slot = self.0.lock().unwrap();
        if slot.as_ref().is_some_and(|(current, _)| *current == id) {
            slot.take().map(|(_, session)| session)
        } else {
            None
        }
    }

    pub fn cancel(&self, id: u64) {
        drop(self.take(id));
    }
}

pub(crate) struct LiveSession {
    pub settings: AppSettings,
    finish: Option<oneshot::Sender<Vec<i16>>>,
    task: JoinHandle<Result<String, String>>,
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl LiveSession {
    fn start(id: u64, audio: AudioState, settings: AppSettings) -> Self {
        let (tx, rx) = oneshot::channel();
        let snapshot = settings.clone();
        let task = tokio::spawn(async move {
            tokio::time::timeout(
                SESSION_TIMEOUT,
                stream(soniox::ENDPOINT, snapshot, rx, move |offset| {
                    let audio = audio.clone();
                    async move { audio.read_since(id, offset).await }
                }),
            )
            .await
            .map_err(|_| "Soniox live session timed out. Try recording again.".to_string())?
        });
        Self {
            settings,
            finish: Some(tx),
            task,
        }
    }

    pub fn end_capture(&mut self, samples: Vec<i16>) {
        if let Some(tx) = self.finish.take() {
            let _ = tx.send(samples);
        }
    }

    pub async fn finish(mut self) -> Result<String, String> {
        tokio::time::timeout(FINALIZE_TIMEOUT, &mut self.task)
            .await
            .map_err(|_| "Soniox finalization timed out. Try again.".to_string())?
            .map_err(|_| "Soniox live session stopped. Try again.".to_string())?
    }
}

fn live_configuration(settings: &AppSettings) -> serde_json::Value {
    let terms = crate::glossary::transcription_keywords(&settings.replacements);
    let prompt = crate::glossary::transcription_context_prompt(&settings.language);
    let request = TranscriptionRequest {
        model: &settings.model,
        provider: "soniox",
        language: Some(&settings.language),
        prompt: prompt.as_deref(),
        keywords: &terms,
        mime_type: "audio/pcm",
        file_name: "audio.pcm",
        audio_duration: Duration::ZERO,
    };
    let mut config = soniox::configuration(&settings.soniox_api_key, &request);
    config["audio_format"] = "pcm_s16le".into();
    config["sample_rate"] = 16000.into();
    config["num_channels"] = 1.into();
    config
}

// At most 200 ms of PCM per frame, preserving sample order and the release tail.
struct AudioCursor {
    sent: usize,
    sensitivity: u8,
    denoiser: Option<crate::audio::StreamingNoiseSuppressor>,
}

impl AudioCursor {
    fn finish(&mut self) -> Vec<u8> {
        self.denoiser
            .as_mut()
            .map(|denoiser| {
                denoiser
                    .finish()
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn new(settings: &AppSettings) -> Self {
        Self {
            sent: 0,
            sensitivity: settings.mic_sensitivity,
            denoiser: settings
                .noise_suppression_enabled
                .then(crate::audio::StreamingNoiseSuppressor::new),
        }
    }

    fn encode(&mut self, mut samples: Vec<i16>) -> Vec<u8> {
        self.sent += samples.len();
        crate::mic_analysis::normalize_quiet_audio(&mut samples, self.sensitivity);
        if let Some(denoiser) = self.denoiser.as_mut() {
            samples = denoiser.push(&samples);
        }
        samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect()
    }
}

async fn stream<F, Fut>(
    endpoint: &str,
    settings: AppSettings,
    mut finish: oneshot::Receiver<Vec<i16>>,
    mut read: F,
) -> Result<String, String>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<i16>, String>>,
{
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_API_RESPONSE_BYTES))
        .max_frame_size(Some(MAX_API_RESPONSE_BYTES));
    let (mut socket, _) = connect_async_with_config(endpoint, Some(config), true)
        .await
        .map_err(soniox::connection_error)?;
    socket
        .send(Message::Text(
            live_configuration(&settings).to_string().into(),
        ))
        .await
        .map_err(soniox::connection_error)?;
    let (mut sender, mut receiver) = socket.split();
    let send = async {
        let mut cursor = AudioCursor::new(&settings);
        let mut tick = tokio::time::interval(Duration::from_millis(20));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                result = &mut finish => {
                    let samples = result.map_err(|_| "Soniox recording cancelled".to_string())?;
                    let tail = samples.get(cursor.sent..)
                        .ok_or_else(|| "Soniox audio sequence was interrupted. Try again.".to_string())?;
                    for chunk in tail.chunks(3200) {
                        let bytes = cursor.encode(chunk.to_vec());
                        if !bytes.is_empty() { sender.send(Message::Binary(bytes.into())).await
                            .map_err(soniox::connection_error)?;
                        }
                    }
                    let tail = cursor.finish();
                    if !tail.is_empty() { sender.send(Message::Binary(tail.into())).await.map_err(soniox::connection_error)?; }
                    sender.send(Message::Text("".into())).await.map_err(soniox::connection_error)?;
                    return Ok::<_, String>(());
                }
                samples = async { tick.tick().await; read(cursor.sent).await } => {
                    let samples = samples?;
                    if !samples.is_empty() {
                        let bytes = cursor.encode(samples);
                        if !bytes.is_empty() { sender.send(Message::Binary(bytes.into())).await
                            .map_err(soniox::connection_error)?;
                        }
                    }
                }
            }
        }
    };
    let receive = async {
        let mut transcript = soniox::Transcript::default();
        while let Some(message) = receiver.next().await {
            match message.map_err(soniox::connection_error)? {
                Message::Text(message) => {
                    if let Some(text) = transcript.push(&message)? {
                        return Ok(text);
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => break,
                _ => return Err(soniox::INVALID_RESPONSE.to_string()),
            }
        }
        Err(soniox::INCOMPLETE_RESPONSE.to_string())
    };
    let (_, text) = tokio::try_join!(send, receive)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::net::TcpListener;

    fn settings() -> AppSettings {
        AppSettings {
            transcription_provider: "soniox".into(),
            model: "stt-rt-v5".into(),
            soniox_api_key: "synthetic-key".into(),
            language: "pt".into(),
            noise_suppression_enabled: false,
            ..AppSettings::default()
        }
    }

    #[tokio::test]
    async fn sends_during_capture_and_preserves_tail_without_duplicate_samples() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let (received_tx, received_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let config: serde_json::Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(config["audio_format"], "pcm_s16le");
            assert_eq!(config["sample_rate"], 16000);
            assert_eq!(config["num_channels"], 1);
            assert_eq!(config["language_hints"], serde_json::json!(["pt", "en"]));
            let mut received = Vec::new();
            let mut signal = Some(received_tx);
            loop {
                match socket.next().await.unwrap().unwrap() {
                    Message::Binary(bytes) => {
                        assert!(!bytes.is_empty());
                        received.extend_from_slice(&bytes);
                        if let Some(tx) = signal.take() {
                            socket
                                .send(Message::Text(
                                    r#"{"tokens":[{"text":"provisional","is_final":false}]}"#
                                        .into(),
                                ))
                                .await
                                .unwrap();
                            tx.send(()).unwrap();
                        }
                    }
                    Message::Text(text) if text.is_empty() => break,
                    _ => panic!("Unexpected frame"),
                }
            }
            socket
                .send(Message::Text(
                    r#"{"tokens":[{"text":"Olá deploy","is_final":true}],"finished":true}"#.into(),
                ))
                .await
                .unwrap();
            received
        });
        let samples: Vec<i16> = (0..6537)
            .map(|i| if i % 2 == 0 { 6000 } else { -6000 })
            .collect();
        let source = samples.clone();
        let (tx, rx) = oneshot::channel();
        let cursor = Arc::new(AtomicUsize::new(0));
        let observed = cursor.clone();
        let task = tokio::spawn(async move {
            stream(&endpoint, settings(), rx, move |offset| {
                observed.store(offset, Ordering::SeqCst);
                let chunk = if offset == 0 {
                    source[..3200].to_vec()
                } else {
                    Vec::new()
                };
                async move { Ok(chunk) }
            })
            .await
        });
        tokio::time::timeout(Duration::from_secs(3), received_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(!task.is_finished(), "Must not deliver before capture ends");
        tx.send(samples.clone()).unwrap();
        assert_eq!(task.await.unwrap().unwrap(), "Olá deploy");
        let expected: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(server.await.unwrap(), expected);
    }

    #[tokio::test]
    async fn dropping_session_aborts_socket_without_finalizing_or_replaying() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let (ready_tx, ready_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let _ = socket.next().await.unwrap().unwrap();
            ready_tx.send(()).unwrap();
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(3), socket.next()).await,
                Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_))))
            ));
        });
        let (tx, rx) = oneshot::channel();
        let session = LiveSession {
            settings: settings(),
            finish: Some(tx),
            task: tokio::spawn(async move {
                stream(&endpoint, settings(), rx, |_| async { Ok(Vec::new()) }).await
            }),
        };
        ready_rx.await.unwrap();
        drop(session);
        server.await.unwrap();
    }

    #[test]
    fn pcm_processing_preserves_count_and_bounds_with_noise_suppression() {
        let mut config = settings();
        config.noise_suppression_enabled = true;
        let mut cursor = AudioCursor::new(&config);
        let mut bytes = 0;
        for length in [3200, 3200, 137] {
            bytes += cursor.encode(vec![200; length]).len();
        }
        bytes += cursor.finish().len();
        assert_eq!(bytes, 6537 * 2);
        assert_eq!(cursor.sent, 6537);
    }

    #[tokio::test]
    async fn disconnection_during_capture_never_delivers_partial_text() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let _ = socket.next().await.unwrap().unwrap();
            socket
                .send(Message::Text(
                    r#"{"tokens":[{"text":"private-partial","is_final":true}]}"#.into(),
                ))
                .await
                .unwrap();
            socket.close(None).await.unwrap();
        });
        let (_tx, rx) = oneshot::channel();
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            stream(&endpoint, settings(), rx, |_| async { Ok(Vec::new()) }),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error, soniox::INCOMPLETE_RESPONSE);
        assert!(!error.contains("private-partial"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stale_session_cannot_take_or_cancel_the_new_recording() {
        let state = LiveState::default();
        let (tx, _rx) = oneshot::channel();
        let task = tokio::spawn(std::future::pending());
        let handle = task.abort_handle();
        *state.0.lock().unwrap() = Some((
            2,
            LiveSession {
                settings: settings(),
                finish: Some(tx),
                task,
            },
        ));
        assert!(state.take(1).is_none());
        state.cancel(1);
        assert!(!handle.is_finished());
        let session = state.take(2).unwrap();
        assert!(state.take(2).is_none());
        drop(session);
        tokio::task::yield_now().await;
        assert!(handle.is_finished());
    }
}
