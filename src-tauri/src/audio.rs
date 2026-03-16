use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// Target sample rate for OpenAI transcription models (16kHz is optimal)
const TARGET_SAMPLE_RATE: u32 = 16000;

/// Pre-allocated buffer capacity: ~60 seconds at 16kHz mono
const INITIAL_SAMPLE_CAPACITY: usize = TARGET_SAMPLE_RATE as usize * 60;

pub struct AudioState {
    pub is_recording: Arc<AtomicBool>,
    pub cmd_tx: mpsc::Sender<AudioCommand>,
}

pub enum AudioCommand {
    Start(tokio::sync::oneshot::Sender<Result<(), String>>),
    Stop(tokio::sync::oneshot::Sender<Option<Vec<i16>>>),
}

/// Single-pole IIR low-pass filter for anti-aliasing before downsampling.
/// Prevents high-frequency aliasing artifacts that degrade transcription accuracy.
struct LowPassFilter {
    prev: f64,
    alpha: f64,
}

impl LowPassFilter {
    fn new(cutoff_hz: f64, sample_rate: f64) -> Self {
        let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate;
        let alpha = dt / (rc + dt);
        Self { prev: 0.0, alpha }
    }

    #[inline]
    fn process(&mut self, sample: f64) -> f64 {
        self.prev += self.alpha * (sample - self.prev);
        self.prev
    }
}

impl Default for AudioState {
    fn default() -> Self {
        let (tx, mut rx) = mpsc::channel::<AudioCommand>(10);
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_recording_clone = is_recording.clone();

        std::thread::spawn(move || {
            let mut stream: Option<cpal::Stream> = None;
            // In-memory sample accumulator — pre-allocated, reused across recordings
            let sample_buffer: Arc<Mutex<Vec<i16>>> =
                Arc::new(Mutex::new(Vec::with_capacity(INITIAL_SAMPLE_CAPACITY)));

            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    AudioCommand::Start(reply) => {
                        // Clear buffer but keep allocation from previous recording
                        sample_buffer.lock().unwrap().clear();

                        let host = cpal::default_host();
                        let device = match host.default_input_device() {
                            Some(d) => d,
                            None => {
                                eprintln!("[FamVoice] No default input device found");
                                let _ = reply.send(Err(
                                    "No microphone found. Check your audio input device.".into(),
                                ));
                                continue;
                            }
                        };

                        let default_config = match device.default_input_config() {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("[FamVoice] Failed to get input config: {}", e);
                                let _ =
                                    reply.send(Err(format!("Microphone config error: {}", e)));
                                continue;
                            }
                        };

                        let sample_format = default_config.sample_format();
                        let stream_config: cpal::StreamConfig = default_config.into();
                        let capture_rate = stream_config.sample_rate.0;
                        let capture_channels = stream_config.channels as usize;
                        let downsample_ratio = capture_rate as f64 / TARGET_SAMPLE_RATE as f64;

                        eprintln!(
                            "[FamVoice] Mic: {}Hz {}ch {:?} → {}Hz mono (ratio {:.2})",
                            capture_rate, capture_channels, sample_format, TARGET_SAMPLE_RATE,
                            downsample_ratio
                        );

                        // Anti-aliasing filter at 87.5% of target Nyquist (7kHz for 16kHz target)
                        let filter_cutoff = (TARGET_SAMPLE_RATE as f64 / 2.0) * 0.875;

                        let err_fn =
                            move |err| eprintln!("[FamVoice] Audio stream error: {}", err);
                        let buffer_clone = sample_buffer.clone();

                        let new_stream = match sample_format {
                            cpal::SampleFormat::I16 => {
                                // Pre-allocate working buffers — moved into closure, reused every callback
                                let mut mono_buf: Vec<i16> = Vec::with_capacity(8192);
                                let mut resampled_buf: Vec<i16> =
                                    Vec::with_capacity((8192.0 / downsample_ratio) as usize + 16);
                                let mut filter =
                                    LowPassFilter::new(filter_cutoff, capture_rate as f64);

                                device.build_input_stream(
                                    &stream_config,
                                    move |data: &[i16], _: &_| {
                                        mono_buf.clear();
                                        resampled_buf.clear();

                                        // Downmix to mono
                                        if capture_channels == 1 {
                                            mono_buf.extend_from_slice(data);
                                        } else {
                                            for frame in data.chunks_exact(capture_channels) {
                                                let sum: i32 =
                                                    frame.iter().map(|&s| s as i32).sum();
                                                mono_buf.push(
                                                    (sum / capture_channels as i32) as i16,
                                                );
                                            }
                                        }

                                        // Anti-alias filter + downsample
                                        if downsample_ratio <= 1.0 {
                                            resampled_buf.extend_from_slice(&mono_buf);
                                        } else {
                                            let mut pos = 0.0f64;
                                            while (pos as usize) < mono_buf.len() {
                                                let filtered = filter
                                                    .process(mono_buf[pos as usize] as f64);
                                                resampled_buf.push(
                                                    filtered.clamp(-32768.0, 32767.0) as i16,
                                                );
                                                pos += downsample_ratio;
                                            }
                                        }

                                        if let Ok(mut buf) = buffer_clone.lock() {
                                            buf.extend_from_slice(&resampled_buf);
                                        }
                                    },
                                    err_fn,
                                    None,
                                )
                            }
                            cpal::SampleFormat::F32 => {
                                let mut mono_buf: Vec<i16> = Vec::with_capacity(8192);
                                let mut resampled_buf: Vec<i16> =
                                    Vec::with_capacity((8192.0 / downsample_ratio) as usize + 16);
                                let mut filter =
                                    LowPassFilter::new(filter_cutoff, capture_rate as f64);

                                device.build_input_stream(
                                    &stream_config,
                                    move |data: &[f32], _: &_| {
                                        mono_buf.clear();
                                        resampled_buf.clear();

                                        // f32→i16 conversion + downmix in a single pass
                                        if capture_channels == 1 {
                                            for &s in data {
                                                mono_buf.push(
                                                    (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16,
                                                );
                                            }
                                        } else {
                                            for frame in data.chunks_exact(capture_channels) {
                                                let sum: f32 = frame.iter().sum();
                                                let avg = (sum / capture_channels as f32)
                                                    .clamp(-1.0, 1.0);
                                                mono_buf
                                                    .push((avg * i16::MAX as f32) as i16);
                                            }
                                        }

                                        if downsample_ratio <= 1.0 {
                                            resampled_buf.extend_from_slice(&mono_buf);
                                        } else {
                                            let mut pos = 0.0f64;
                                            while (pos as usize) < mono_buf.len() {
                                                let filtered = filter
                                                    .process(mono_buf[pos as usize] as f64);
                                                resampled_buf.push(
                                                    filtered.clamp(-32768.0, 32767.0) as i16,
                                                );
                                                pos += downsample_ratio;
                                            }
                                        }

                                        if let Ok(mut buf) = buffer_clone.lock() {
                                            buf.extend_from_slice(&resampled_buf);
                                        }
                                    },
                                    err_fn,
                                    None,
                                )
                            }
                            other => {
                                eprintln!("[FamVoice] Unsupported sample format: {:?}", other);
                                let _ = reply
                                    .send(Err(format!("Unsupported audio format: {:?}", other)));
                                continue;
                            }
                        };

                        match new_stream {
                            Ok(s) => {
                                if let Err(e) = s.play() {
                                    eprintln!("[FamVoice] Failed to start audio stream: {}", e);
                                    let _ = reply.send(Err(format!(
                                        "Failed to start recording: {}",
                                        e
                                    )));
                                    continue;
                                }
                                stream = Some(s);
                                is_recording_clone.store(true, Ordering::SeqCst);
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                eprintln!("[FamVoice] Failed to build input stream: {}", e);
                                let _ =
                                    reply.send(Err(format!("Failed to open microphone: {}", e)));
                            }
                        }
                    }
                    AudioCommand::Stop(reply) => {
                        // Drop stream first to ensure no more callbacks fire
                        stream.take();
                        is_recording_clone.store(false, Ordering::SeqCst);

                        let samples = sample_buffer.lock().unwrap().clone();
                        if samples.is_empty() {
                            let _ = reply.send(None);
                        } else {
                            eprintln!(
                                "[FamVoice] Captured {} samples ({:.1}s)",
                                samples.len(),
                                samples.len() as f64 / TARGET_SAMPLE_RATE as f64
                            );
                            let _ = reply.send(Some(samples));
                        }
                    }
                }
            }
        });

        Self {
            is_recording,
            cmd_tx: tx,
        }
    }
}

/// Encode PCM samples as WAV bytes in memory (16kHz, mono, 16-bit).
/// Builds the 44-byte header directly — no file I/O, no external crate needed.
pub fn encode_wav_in_memory(samples: &[i16]) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;
    let mut buf = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt sub-chunk (16 bytes for PCM)
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&1u16.to_le_bytes()); // 1 channel (mono)
    buf.extend_from_slice(&TARGET_SAMPLE_RATE.to_le_bytes());
    buf.extend_from_slice(&(TARGET_SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data sub-chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }

    buf
}

pub async fn start_recording(state: &AudioState) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .cmd_tx
        .send(AudioCommand::Start(tx))
        .await
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|e| format!("Recording channel error: {}", e))?
}

pub async fn stop_recording(state: &AudioState) -> Option<Vec<i16>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = state.cmd_tx.send(AudioCommand::Stop(tx)).await;
    rx.await.unwrap_or(None)
}
