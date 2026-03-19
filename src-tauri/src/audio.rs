use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Target sample rate for OpenAI transcription models (16kHz is optimal)
const TARGET_SAMPLE_RATE: u32 = 16000;

/// Pre-allocated buffer capacity: ~60 seconds at 16kHz mono
const INITIAL_SAMPLE_CAPACITY: usize = TARGET_SAMPLE_RATE as usize * 60;
const SPEECH_WINDOW_FRAME_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 20) / 1000;
const SPEECH_WINDOW_MIN_SPEECH_FRAMES: usize = 3;
const SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 300) / 1000;
const SPEECH_WINDOW_MIN_CLIP_SAMPLES: usize = TARGET_SAMPLE_RATE as usize;
const SPEECH_WINDOW_MIN_TRIMMED_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 4;
const SPEECH_WINDOW_MIN_SAVED_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioChunkRoute {
    Drop,
    BufferOnly,
    BufferAndStream,
}

pub fn audio_chunk_route(is_recording: bool, streaming_enabled: bool) -> AudioChunkRoute {
    match (is_recording, streaming_enabled) {
        (false, _) => AudioChunkRoute::Drop,
        (true, true) => AudioChunkRoute::BufferAndStream,
        (true, false) => AudioChunkRoute::BufferOnly,
    }
}

pub struct AudioState {
    pub is_recording: Arc<AtomicBool>,
    pub cmd_tx: mpsc::Sender<AudioCommand>,
    chunk_tx: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<i16>>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadAudioSelection {
    pub samples: Vec<i16>,
    pub was_trimmed: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordingCycleState {
    armed: bool,
    is_recording: bool,
}

fn begin_recording_cycle(state: &mut RecordingCycleState, buffer: &mut Vec<i16>) {
    buffer.clear();
    state.armed = true;
    state.is_recording = true;
}

fn finish_recording_cycle(
    state: &mut RecordingCycleState,
    buffer: &mut Vec<i16>,
) -> Option<Vec<i16>> {
    state.armed = false;
    state.is_recording = false;
    take_recorded_samples(buffer)
}

fn start_persistent_input_stream(
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    chunk_tx: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<i16>>>>>,
    armed: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    needs_rebuild: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No microphone found. Check your audio input device.".to_string())?;

    let default_config = device
        .default_input_config()
        .map_err(|e| format!("Microphone config error: {}", e))?;

    let sample_format = default_config.sample_format();
    let stream_config: cpal::StreamConfig = default_config.into();
    let capture_rate = stream_config.sample_rate.0;
    let capture_channels = stream_config.channels as usize;
    let downsample_ratio = capture_rate as f64 / TARGET_SAMPLE_RATE as f64;

    eprintln!(
        "[FamVoice] Mic: {}Hz {}ch {:?} -> {}Hz mono (ratio {:.2})",
        capture_rate, capture_channels, sample_format, TARGET_SAMPLE_RATE, downsample_ratio
    );

    let filter_cutoff = (TARGET_SAMPLE_RATE as f64 / 2.0) * 0.875;

    let err_armed = armed.clone();
    let err_recording = is_recording.clone();
    let err_rebuild = needs_rebuild.clone();
    let err_fn = move |err| {
        eprintln!("[FamVoice] Audio stream error: {}", err);
        err_armed.store(false, Ordering::SeqCst);
        err_recording.store(false, Ordering::SeqCst);
        err_rebuild.store(true, Ordering::SeqCst);
    };

    let stream = match sample_format {
        cpal::SampleFormat::I16 => {
            let mut mono_buf: Vec<i16> = Vec::with_capacity(8192);
            let mut resampled_buf: Vec<i16> =
                Vec::with_capacity((8192.0 / downsample_ratio) as usize + 16);
            let mut filter = LowPassFilter::new(filter_cutoff, capture_rate as f64);
            let buffer_clone = sample_buffer.clone();
            let chunk_tx_clone = chunk_tx.clone();
            let armed_clone = armed.clone();

            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &_| {
                    if !armed_clone.load(Ordering::SeqCst) {
                        return;
                    }

                    mono_buf.clear();
                    resampled_buf.clear();

                    if capture_channels == 1 {
                        mono_buf.extend_from_slice(data);
                    } else {
                        for frame in data.chunks_exact(capture_channels) {
                            let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                            mono_buf.push((sum / capture_channels as i32) as i16);
                        }
                    }

                    if downsample_ratio <= 1.0 {
                        resampled_buf.extend_from_slice(&mono_buf);
                    } else {
                        let mut pos = 0.0f64;
                        while (pos as usize) < mono_buf.len() {
                            let filtered = filter.process(mono_buf[pos as usize] as f64);
                            resampled_buf.push(filtered.clamp(-32768.0, 32767.0) as i16);
                            pos += downsample_ratio;
                        }
                    }

                    let streaming_enabled = chunk_tx_clone
                        .lock()
                        .ok()
                        .and_then(|sender| sender.as_ref().cloned())
                        .is_some();

                    match audio_chunk_route(true, streaming_enabled) {
                        AudioChunkRoute::Drop => {}
                        AudioChunkRoute::BufferOnly => {
                            if let Ok(mut buf) = buffer_clone.lock() {
                                buf.extend_from_slice(&resampled_buf);
                            }
                        }
                        AudioChunkRoute::BufferAndStream => {
                            if let Ok(mut buf) = buffer_clone.lock() {
                                buf.extend_from_slice(&resampled_buf);
                            }
                            if let Ok(sender_guard) = chunk_tx_clone.lock() {
                                if let Some(sender) = sender_guard.as_ref() {
                                    let _ = sender.send(resampled_buf.clone());
                                }
                            }
                        }
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
            let mut filter = LowPassFilter::new(filter_cutoff, capture_rate as f64);
            let buffer_clone = sample_buffer.clone();
            let chunk_tx_clone = chunk_tx.clone();
            let armed_clone = armed.clone();

            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &_| {
                    if !armed_clone.load(Ordering::SeqCst) {
                        return;
                    }

                    mono_buf.clear();
                    resampled_buf.clear();

                    if capture_channels == 1 {
                        for &s in data {
                            mono_buf.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                        }
                    } else {
                        for frame in data.chunks_exact(capture_channels) {
                            let sum: f32 = frame.iter().sum();
                            let avg = (sum / capture_channels as f32).clamp(-1.0, 1.0);
                            mono_buf.push((avg * i16::MAX as f32) as i16);
                        }
                    }

                    if downsample_ratio <= 1.0 {
                        resampled_buf.extend_from_slice(&mono_buf);
                    } else {
                        let mut pos = 0.0f64;
                        while (pos as usize) < mono_buf.len() {
                            let filtered = filter.process(mono_buf[pos as usize] as f64);
                            resampled_buf.push(filtered.clamp(-32768.0, 32767.0) as i16);
                            pos += downsample_ratio;
                        }
                    }

                    let streaming_enabled = chunk_tx_clone
                        .lock()
                        .ok()
                        .and_then(|sender| sender.as_ref().cloned())
                        .is_some();

                    match audio_chunk_route(true, streaming_enabled) {
                        AudioChunkRoute::Drop => {}
                        AudioChunkRoute::BufferOnly => {
                            if let Ok(mut buf) = buffer_clone.lock() {
                                buf.extend_from_slice(&resampled_buf);
                            }
                        }
                        AudioChunkRoute::BufferAndStream => {
                            if let Ok(mut buf) = buffer_clone.lock() {
                                buf.extend_from_slice(&resampled_buf);
                            }
                            if let Ok(sender_guard) = chunk_tx_clone.lock() {
                                if let Some(sender) = sender_guard.as_ref() {
                                    let _ = sender.send(resampled_buf.clone());
                                }
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        other => {
            return Err(format!("Unsupported audio format: {:?}", other));
        }
    }
    .map_err(|e| format!("Failed to open microphone: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    Ok(stream)
}

impl Default for AudioState {
    fn default() -> Self {
        let (tx, mut rx) = mpsc::channel::<AudioCommand>(10);
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_recording_clone = is_recording.clone();
        let armed = Arc::new(AtomicBool::new(false));
        let armed_clone = armed.clone();
        let needs_rebuild = Arc::new(AtomicBool::new(false));
        let needs_rebuild_clone = needs_rebuild.clone();
        let chunk_tx: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<i16>>>>> =
            Arc::new(Mutex::new(None));
        let chunk_tx_clone = chunk_tx.clone();

        std::thread::spawn(move || {
            let sample_buffer: Arc<Mutex<Vec<i16>>> =
                Arc::new(Mutex::new(Vec::with_capacity(INITIAL_SAMPLE_CAPACITY)));
            let mut recording_state = RecordingCycleState {
                armed: false,
                is_recording: false,
            };
            let mut stream = match start_persistent_input_stream(
                sample_buffer.clone(),
                chunk_tx_clone.clone(),
                armed_clone.clone(),
                is_recording_clone.clone(),
                needs_rebuild_clone.clone(),
            ) {
                Ok(stream) => Some(stream),
                Err(error) => {
                    eprintln!(
                        "[FamVoice] Persistent microphone stream unavailable at startup: {}",
                        error
                    );
                    needs_rebuild_clone.store(true, Ordering::SeqCst);
                    None
                }
            };

            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    AudioCommand::Start(reply) => {
                        if needs_rebuild_clone.swap(false, Ordering::SeqCst) {
                            stream.take();
                        }

                        if stream.is_none() {
                            match start_persistent_input_stream(
                                sample_buffer.clone(),
                                chunk_tx_clone.clone(),
                                armed_clone.clone(),
                                is_recording_clone.clone(),
                                needs_rebuild_clone.clone(),
                            ) {
                                Ok(new_stream) => {
                                    stream = Some(new_stream);
                                }
                                Err(error) => {
                                    let _ = reply.send(Err(error));
                                    continue;
                                }
                            }
                        }

                        {
                            let mut buffer = sample_buffer.lock().unwrap();
                            begin_recording_cycle(&mut recording_state, &mut buffer);
                        }
                        armed_clone.store(recording_state.armed, Ordering::SeqCst);
                        is_recording_clone.store(recording_state.is_recording, Ordering::SeqCst);
                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Stop(reply) => {
                        let samples = {
                            let mut buffer = sample_buffer.lock().unwrap();
                            finish_recording_cycle(&mut recording_state, &mut buffer)
                        };
                        armed_clone.store(recording_state.armed, Ordering::SeqCst);
                        is_recording_clone.store(recording_state.is_recording, Ordering::SeqCst);

                        if let Some(samples) = samples {
                            eprintln!(
                                "[FamVoice] Captured {} samples ({:.1}s)",
                                samples.len(),
                                samples.len() as f64 / TARGET_SAMPLE_RATE as f64
                            );
                            let _ = reply.send(Some(samples));
                        } else {
                            let _ = reply.send(None);
                        }
                    }
                }
            }
        });

        Self {
            is_recording,
            cmd_tx: tx,
            chunk_tx,
        }
    }
}

/// Encode PCM samples as WAV bytes in memory (16kHz, mono, 16-bit).
/// Builds the 44-byte header directly - no file I/O, no external crate needed.
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

fn take_recorded_samples(buffer: &mut Vec<i16>) -> Option<Vec<i16>> {
    if buffer.is_empty() {
        return None;
    }

    let mut taken = Vec::with_capacity(buffer.capacity());
    std::mem::swap(buffer, &mut taken);
    Some(taken)
}

fn frame_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut sum_squares = 0.0;
    for &sample in samples {
        let sample = sample as f64;
        sum_squares += sample * sample;
    }

    (sum_squares / samples.len() as f64).sqrt()
}

fn speech_frame_levels(samples: &[i16]) -> Vec<f64> {
    samples
        .chunks(SPEECH_WINDOW_FRAME_SAMPLES)
        .map(frame_rms)
        .collect()
}

fn speech_window_start_frame(frame_levels: &[f64], threshold: f64) -> Option<usize> {
    let mut consecutive = 0usize;

    for (index, level) in frame_levels.iter().enumerate() {
        if *level >= threshold {
            consecutive += 1;
            if consecutive >= SPEECH_WINDOW_MIN_SPEECH_FRAMES {
                return Some(index + 1 - consecutive);
            }
        } else {
            consecutive = 0;
        }
    }

    None
}

fn speech_window_end_frame(frame_levels: &[f64], threshold: f64, start_frame: usize) -> Option<usize> {
    let mut consecutive = 0usize;

    for index in (start_frame..frame_levels.len()).rev() {
        if frame_levels[index] >= threshold {
            consecutive += 1;
            if consecutive >= SPEECH_WINDOW_MIN_SPEECH_FRAMES {
                return Some(index + consecutive - 1);
            }
        } else {
            consecutive = 0;
        }
    }

    None
}

pub fn select_samples_for_upload(samples: &[i16], silence_threshold_rms: f64) -> UploadAudioSelection {
    if samples.len() < SPEECH_WINDOW_MIN_CLIP_SAMPLES {
        return UploadAudioSelection {
            samples: samples.to_vec(),
            was_trimmed: false,
        };
    }

    let frame_levels = speech_frame_levels(samples);
    let Some(start_frame) = speech_window_start_frame(&frame_levels, silence_threshold_rms) else {
        return UploadAudioSelection {
            samples: samples.to_vec(),
            was_trimmed: false,
        };
    };
    let Some(end_frame) = speech_window_end_frame(&frame_levels, silence_threshold_rms, start_frame) else {
        return UploadAudioSelection {
            samples: samples.to_vec(),
            was_trimmed: false,
        };
    };

    let start_sample = start_frame * SPEECH_WINDOW_FRAME_SAMPLES;
    let end_sample = (((end_frame + 1) * SPEECH_WINDOW_FRAME_SAMPLES) + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES)
        .min(samples.len());

    if end_sample <= start_sample {
        return UploadAudioSelection {
            samples: samples.to_vec(),
            was_trimmed: false,
        };
    }

    let trimmed_len = end_sample - start_sample;
    let saved_samples = samples.len().saturating_sub(trimmed_len);

    if trimmed_len < SPEECH_WINDOW_MIN_TRIMMED_SAMPLES
        || saved_samples < SPEECH_WINDOW_MIN_SAVED_SAMPLES
        || (start_sample == 0 && end_sample == samples.len())
    {
        return UploadAudioSelection {
            samples: samples.to_vec(),
            was_trimmed: false,
        };
    }

    UploadAudioSelection {
        samples: samples[start_sample..end_sample].to_vec(),
        was_trimmed: true,
    }
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

pub fn set_stream_chunk_sender(
    state: &AudioState,
    sender: Option<mpsc::UnboundedSender<Vec<i16>>>,
) {
    if let Ok(mut current) = state.chunk_tx.lock() {
        *current = sender;
    }
}

pub async fn stop_recording(state: &AudioState) -> Option<Vec<i16>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if state.cmd_tx.send(AudioCommand::Stop(tx)).await.is_err() {
        eprintln!("[FamVoice] Failed to send stop command to audio thread");
        return None;
    }
    match rx.await {
        Ok(samples) => samples,
        Err(e) => {
            eprintln!("[FamVoice] Failed to receive stop response: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_cycle_start_clears_stale_samples_and_arms_capture() {
        let mut samples = vec![7, 8, 9];
        let mut state = RecordingCycleState {
            armed: false,
            is_recording: false,
        };

        begin_recording_cycle(&mut state, &mut samples);

        assert!(samples.is_empty());
        assert_eq!(
            state,
            RecordingCycleState {
                armed: true,
                is_recording: true,
            }
        );
    }

    #[test]
    fn recording_cycle_stop_disarms_capture_and_returns_buffered_samples() {
        let mut samples = vec![1, 2, 3];
        let mut state = RecordingCycleState {
            armed: true,
            is_recording: true,
        };

        let captured = finish_recording_cycle(&mut state, &mut samples);

        assert_eq!(captured, Some(vec![1, 2, 3]));
        assert!(samples.is_empty());
        assert_eq!(
            state,
            RecordingCycleState {
                armed: false,
                is_recording: false,
            }
        );
    }

    #[test]
    fn audio_chunk_routes_to_buffer_and_stream_when_streaming_is_active() {
        let decision = audio_chunk_route(true, true);

        assert_eq!(decision, AudioChunkRoute::BufferAndStream);
    }

    #[test]
    fn audio_chunk_routes_to_buffer_only_when_streaming_is_unavailable() {
        let decision = audio_chunk_route(true, false);

        assert_eq!(decision, AudioChunkRoute::BufferOnly);
    }

    #[test]
    fn audio_chunk_is_dropped_when_recording_is_inactive() {
        let decision = audio_chunk_route(false, true);

        assert_eq!(decision, AudioChunkRoute::Drop);
    }

    #[test]
    fn test_take_recorded_samples_returns_none_for_empty_buffer() {
        let mut samples = Vec::new();

        assert_eq!(take_recorded_samples(&mut samples), None);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_take_recorded_samples_moves_recording_out_of_buffer() {
        let mut samples = vec![1, 2, 3, 4];

        let taken = take_recorded_samples(&mut samples);

        assert_eq!(taken, Some(vec![1, 2, 3, 4]));
        assert!(samples.is_empty());
        samples.push(9);
        assert_eq!(taken, Some(vec![1, 2, 3, 4]));
    }

    fn silence_samples(length: usize) -> Vec<i16> {
        vec![0; length]
    }

    fn speech_samples(length: usize, amplitude: i16) -> Vec<i16> {
        vec![amplitude; length]
    }

    #[test]
    fn speech_window_trims_leading_silence() {
        let mut samples = silence_samples(TARGET_SAMPLE_RATE as usize);
        samples.extend(speech_samples(TARGET_SAMPLE_RATE as usize, 800));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() < samples.len());
        assert_eq!(selected.samples[0], 800);
    }

    #[test]
    fn speech_window_trims_trailing_silence() {
        let mut samples = speech_samples(TARGET_SAMPLE_RATE as usize, 800);
        samples.extend(silence_samples(TARGET_SAMPLE_RATE as usize));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() < samples.len());
        assert_eq!(selected.samples[0], 800);
        assert!(selected.samples.len() <= TARGET_SAMPLE_RATE as usize + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES + SPEECH_WINDOW_FRAME_SAMPLES);
    }

    #[test]
    fn speech_window_trims_both_leading_and_trailing_silence() {
        let mut samples = silence_samples(TARGET_SAMPLE_RATE as usize);
        samples.extend(speech_samples(TARGET_SAMPLE_RATE as usize, 800));
        samples.extend(silence_samples(TARGET_SAMPLE_RATE as usize));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() < samples.len());
        assert_eq!(selected.samples[0], 800);
        assert!(selected.samples.len() <= TARGET_SAMPLE_RATE as usize + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES + SPEECH_WINDOW_FRAME_SAMPLES);
    }

    #[test]
    fn speech_window_keeps_short_clips_untrimmed() {
        let mut samples = silence_samples(SPEECH_WINDOW_FRAME_SAMPLES * 2);
        samples.extend(speech_samples(SPEECH_WINDOW_FRAME_SAMPLES * 8, 800));
        samples.extend(silence_samples(SPEECH_WINDOW_FRAME_SAMPLES * 2));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(!selected.was_trimmed);
        assert_eq!(selected.samples, samples);
    }

    #[test]
    fn speech_window_keeps_trailing_context_after_last_speech_frame() {
        let mut samples = silence_samples(TARGET_SAMPLE_RATE as usize);
        samples.extend(speech_samples(SPEECH_WINDOW_FRAME_SAMPLES * 4, 800));
        samples.extend(silence_samples(SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES / 2));
        samples.extend(silence_samples(TARGET_SAMPLE_RATE as usize));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() > SPEECH_WINDOW_FRAME_SAMPLES * 4);
        assert!(selected.samples.len() < samples.len());
    }
}
