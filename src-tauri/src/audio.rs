mod noise;

pub use noise::maybe_apply_noise_suppression;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::mpsc;

/// Target sample rate for OpenAI transcription models (16kHz is optimal)
const TARGET_SAMPLE_RATE: u32 = 16000;
const MAX_RECORDING_DURATION_SECONDS: usize = 5 * 60;
const MAX_RECORDED_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * MAX_RECORDING_DURATION_SECONDS;
const LOW_LATENCY_TARGET_BUFFER_MS: u32 = 10;

const PREROLL_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 500) / 1000;

const SPEECH_WINDOW_FRAME_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 20) / 1000;
const SPEECH_WINDOW_MIN_SPEECH_FRAMES: usize = 3;
const SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 200) / 1000;
const SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * 300) / 1000;
const SPEECH_WINDOW_MIN_CLIP_SAMPLES: usize = TARGET_SAMPLE_RATE as usize;
const SPEECH_WINDOW_MIN_TRIMMED_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 4;
const SPEECH_WINDOW_MIN_SAVED_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 10;

#[derive(Clone)]
pub struct AudioState {
    pub is_recording: Arc<AtomicBool>,
    pub cmd_tx: mpsc::Sender<AudioCommand>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputDeviceOption {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadAudioSelection<'a> {
    pub samples: Cow<'a, [i16]>,
    pub was_trimmed: bool,
}

pub enum AudioCommand {
    Prime(
        tauri::AppHandle,
        Option<String>,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    ),
    Start(
        tauri::AppHandle,
        Option<String>,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    ),
    Stop(tokio::sync::oneshot::Sender<Option<Vec<i16>>>),
}

pub fn list_input_devices() -> Result<Vec<InputDeviceOption>, String> {
    let host = cpal::default_host();
    list_input_devices_for_host(&host)
}

fn list_input_devices_for_host(host: &cpal::Host) -> Result<Vec<InputDeviceOption>, String> {
    let default_device_id = host
        .default_input_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());

    let mut devices = Vec::new();
    let input_devices = host
        .input_devices()
        .map_err(|error| format!("Failed to enumerate input devices: {}", error))?;

    for device in input_devices {
        let id = match device.id() {
            Ok(id) => id.to_string(),
            Err(error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[FamVoice] Skipping input device without stable id: {}",
                    error
                );
                continue;
            }
        };

        let label = input_device_label(&device);
        let is_default = default_device_id
            .as_ref()
            .is_some_and(|default_id| default_id == &id);

        devices.push(InputDeviceOption {
            id,
            label,
            is_default,
        });
    }

    devices.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(devices)
}

fn input_device_candidates(
    host: &cpal::Host,
    selected_device_id: Option<&str>,
) -> Result<Vec<cpal::Device>, String> {
    let mut candidates = Vec::new();
    let normalized_selection = selected_device_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);

    if let Some(selection) = normalized_selection.as_deref() {
        if let Ok(parsed_id) = cpal::DeviceId::from_str(selection) {
            if let Some(device) = host.device_by_id(&parsed_id) {
                candidates.push(device);
            }
        }
    }

    if let Some(default_device) = host.default_input_device() {
        let default_device_id = default_device.id().ok().map(|id| id.to_string());
        if normalized_selection.as_deref() != default_device_id.as_deref() {
            candidates.push(default_device);
        }
    }

    if candidates.is_empty() {
        let mut fallback_devices = host
            .input_devices()
            .map_err(|error| format!("Failed to enumerate input devices: {}", error))?;
        if let Some(device) = fallback_devices.next() {
            candidates.push(device);
        }
    }

    if candidates.is_empty() {
        Err("No microphone found. Check your audio input device.".to_string())
    } else {
        Ok(candidates)
    }
}

fn input_device_label(device: &cpal::Device) -> String {
    match device.description() {
        Ok(description) => {
            let label = description.to_string();
            if label.trim().is_empty() {
                device
                    .id()
                    .map(|id| format!("Microphone {}", id))
                    .unwrap_or_else(|_| "Unknown microphone".to_string())
            } else {
                label
            }
        }
        Err(_) => device
            .id()
            .map(|id| format!("Microphone {}", id))
            .unwrap_or_else(|_| "Unknown microphone".to_string()),
    }
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
    pending_start: bool,
    armed: bool,
    is_recording: bool,
}

fn prepare_recording_cycle(
    state: &mut RecordingCycleState,
    buffer: &mut Vec<i16>,
    preroll: &mut Vec<i16>,
) {
    buffer.clear();
    preroll.clear();
    state.pending_start = true;
    state.armed = false;
    state.is_recording = true;
}

fn append_samples_capped(buffer: &mut Vec<i16>, new_samples: &[i16]) {
    let remaining_capacity = MAX_RECORDED_SAMPLES.saturating_sub(buffer.len());
    if remaining_capacity == 0 {
        return;
    }

    let samples_to_append = remaining_capacity.min(new_samples.len());
    buffer.extend_from_slice(&new_samples[..samples_to_append]);
}

fn append_preroll_samples_capped(preroll: &mut Vec<i16>, new_samples: &[i16]) {
    preroll.extend_from_slice(new_samples);
    if preroll.len() > PREROLL_SAMPLES {
        let excess = preroll.len() - PREROLL_SAMPLES;
        preroll.drain(..excess);
    }
}

fn promote_preroll_to_recording_buffer(
    buffer: &mut Vec<i16>,
    preroll: &mut Vec<i16>,
    warmup_samples: &[i16],
) {
    append_preroll_samples_capped(preroll, warmup_samples);
    buffer.clear();
    buffer.append(preroll);
}

#[cfg(test)]
fn activate_recording_cycle(
    state: &mut RecordingCycleState,
    buffer: &mut Vec<i16>,
    preroll: &mut Vec<i16>,
    warmup_samples: &[i16],
) {
    promote_preroll_to_recording_buffer(buffer, preroll, warmup_samples);
    state.pending_start = false;
    state.armed = true;
    state.is_recording = true;
}

fn finish_recording_cycle(
    state: &mut RecordingCycleState,
    buffer: &mut Vec<i16>,
) -> Option<Vec<i16>> {
    state.pending_start = false;
    state.armed = false;
    state.is_recording = false;
    take_recorded_samples(buffer)
}

fn mix_down_and_resample<T>(
    data: &[T],
    capture_channels: usize,
    downsample_ratio: f64,
    filter: &mut LowPassFilter,
    mono_buf: &mut Vec<f64>,
    resampled_buf: &mut Vec<i16>,
) where
    T: Sample + Copy,
    i16: cpal::FromSample<T>,
{
    mono_buf.clear();
    resampled_buf.clear();

    if capture_channels == 1 {
        mono_buf.extend(
            data.iter()
                .copied()
                .map(|sample| sample.to_sample::<i16>() as f64),
        );
    } else {
        for frame in data.chunks_exact(capture_channels) {
            let sum: f64 = frame
                .iter()
                .copied()
                .map(|sample| sample.to_sample::<i16>() as f64)
                .sum();
            mono_buf.push(sum / capture_channels as f64);
        }
    }

    if downsample_ratio <= 1.0 {
        resampled_buf.extend(
            mono_buf
                .iter()
                .map(|sample| sample.round().clamp(-32768.0, 32767.0) as i16),
        );
        return;
    }

    let mut pos = 0.0f64;
    while (pos as usize) < mono_buf.len() {
        let filtered = filter.process(mono_buf[pos as usize]);
        resampled_buf.push(filtered.round().clamp(-32768.0, 32767.0) as i16);
        pos += downsample_ratio;
    }
}

fn build_mono_input_stream<T, ErrFn>(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    preroll_buffer: Arc<Mutex<Vec<i16>>>,
    pending_start: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
    capture_channels: usize,
    capture_rate: u32,
    downsample_ratio: f64,
    filter_cutoff: f64,
    err_fn: ErrFn,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: Sample + cpal::SizedSample + Copy + Send + 'static,
    i16: cpal::FromSample<T>,
    ErrFn: FnMut(cpal::StreamError) + Send + 'static,
{
    let mut mono_buf: Vec<f64> = Vec::with_capacity(8192);
    let mut resampled_buf: Vec<i16> = Vec::with_capacity((8192.0 / downsample_ratio) as usize + 16);
    let mut filter = LowPassFilter::new(filter_cutoff, capture_rate as f64);
    let mut last_emit = std::time::Instant::now();
    let mut smoothed_level = 0.0f64;

    device.build_input_stream(
        stream_config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            mix_down_and_resample(
                data,
                capture_channels,
                downsample_ratio,
                &mut filter,
                &mut mono_buf,
                &mut resampled_buf,
            );

            let is_armed = armed.load(Ordering::Acquire);

            if !is_armed {
                if pending_start.load(Ordering::Acquire) {
                    if let Ok(mut buf) = sample_buffer.lock() {
                        if let Ok(mut preroll) = preroll_buffer.lock() {
                            promote_preroll_to_recording_buffer(
                                &mut buf,
                                &mut preroll,
                                &resampled_buf,
                            );
                            pending_start.store(false, Ordering::Release);
                            armed.store(true, Ordering::Release);
                            is_recording.store(true, Ordering::SeqCst);
                        }
                    }
                } else if let Ok(mut buf) = preroll_buffer.lock() {
                    append_preroll_samples_capped(&mut buf, &resampled_buf);
                }
                return;
            }

            if let Ok(mut buf) = sample_buffer.lock() {
                append_samples_capped(&mut buf, &resampled_buf);
            }

            if last_emit.elapsed().as_millis() >= 16 {
                let rms = frame_rms(&resampled_buf);
                // Drastically increased sensitivity (from 2400 to 800)
                let target_level = (rms / 800.0).clamp(0.0, 1.0);

                // Even faster attack (0.8) and slightly faster decay (0.3)
                if target_level > smoothed_level {
                    smoothed_level = 0.8 * target_level + 0.2 * smoothed_level;
                } else {
                    smoothed_level = 0.3 * target_level + 0.7 * smoothed_level;
                }

                if smoothed_level > 0.001 {
                    let _ = app_handle.emit("mic-level", smoothed_level);
                } else if smoothed_level > 0.0 {
                    let _ = app_handle.emit("mic-level", 0.0);
                    smoothed_level = 0.0;
                }

                last_emit = std::time::Instant::now();
            }
        },
        err_fn,
        None,
    )
}

fn preferred_input_buffer_frames(
    config: &cpal::SupportedStreamConfig,
) -> Option<cpal::FrameCount> {
    match config.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            let target_frames =
                (u64::from(config.sample_rate()) * u64::from(LOW_LATENCY_TARGET_BUFFER_MS))
                    / 1000;
            Some(target_frames.clamp(u64::from(*min), u64::from(*max)) as cpal::FrameCount)
        }
        cpal::SupportedBufferSize::Unknown => None,
    }
}

fn build_stream_for_sample_format(
    device: &cpal::Device,
    sample_format: SampleFormat,
    stream_config: &cpal::StreamConfig,
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    preroll_buffer: Arc<Mutex<Vec<i16>>>,
    pending_start: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
    capture_channels: usize,
    capture_rate: u32,
    downsample_ratio: f64,
    filter_cutoff: f64,
    make_err_fn: impl Fn() -> Box<dyn FnMut(cpal::StreamError) + Send + 'static>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    match sample_format {
        SampleFormat::I8 => build_mono_input_stream::<i8, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::I16 => build_mono_input_stream::<i16, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::I24 => build_mono_input_stream::<cpal::I24, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::I32 => build_mono_input_stream::<i32, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::I64 => build_mono_input_stream::<i64, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::U8 => build_mono_input_stream::<u8, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::U16 => build_mono_input_stream::<u16, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::U24 => build_mono_input_stream::<cpal::U24, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::U32 => build_mono_input_stream::<u32, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::U64 => build_mono_input_stream::<u64, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::F32 => build_mono_input_stream::<f32, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        SampleFormat::F64 => build_mono_input_stream::<f64, _>(
            device,
            stream_config,
            sample_buffer,
            preroll_buffer,
            pending_start,
            armed,
            is_recording,
            app_handle,
            capture_channels,
            capture_rate,
            downsample_ratio,
            filter_cutoff,
            make_err_fn(),
        ),
        other => Err(cpal::BuildStreamError::StreamConfigNotSupported).map_err(|_| {
            let _ = other;
            cpal::BuildStreamError::StreamConfigNotSupported
        }),
    }
}

fn build_persistent_input_stream_for_device(
    device: cpal::Device,
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    preroll_buffer: Arc<Mutex<Vec<i16>>>,
    pending_start: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    needs_rebuild: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
) -> Result<(cpal::Stream, String), String> {
    let device_label = input_device_label(&device);
    let device_id = device
        .id()
        .ok()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let default_config = device
        .default_input_config()
        .map_err(|e| format!("Microphone config error: {}", e))?;

    let sample_format = default_config.sample_format();
    let default_stream_config = default_config.config();
    let mut stream_config = default_stream_config.clone();
    let capture_rate = stream_config.sample_rate;
    let capture_channels = stream_config.channels as usize;
    let downsample_ratio = capture_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let requested_buffer_frames = preferred_input_buffer_frames(&default_config);

    if let Some(buffer_frames) = requested_buffer_frames {
        stream_config.buffer_size = cpal::BufferSize::Fixed(buffer_frames);
    }

    let buffer_note = match (default_config.buffer_size(), requested_buffer_frames) {
        (cpal::SupportedBufferSize::Range { min, max }, Some(requested)) => {
            format!("buffer {}-{} frames, request {}", min, max, requested)
        }
        (cpal::SupportedBufferSize::Range { min, max }, None) => {
            format!("buffer {}-{} frames, host default", min, max)
        }
        (cpal::SupportedBufferSize::Unknown, Some(requested)) => {
            format!("buffer unknown, request {}", requested)
        }
        (cpal::SupportedBufferSize::Unknown, None) => "buffer host default".to_string(),
    };

    eprintln!(
        "[FamVoice] Mic: {} [{}] {}Hz {}ch {:?} -> {}Hz mono (ratio {:.2}, {})",
        device_label,
        device_id,
        capture_rate,
        capture_channels,
        sample_format,
        TARGET_SAMPLE_RATE,
        downsample_ratio,
        buffer_note
    );

    let filter_cutoff = (TARGET_SAMPLE_RATE as f64 / 2.0) * 0.875;

    let make_err_fn = || {
        let err_pending_start = pending_start.clone();
        let err_armed = armed.clone();
        let err_recording = is_recording.clone();
        let err_rebuild = needs_rebuild.clone();

        move |err| {
            eprintln!("[FamVoice] Audio stream error: {}", err);
            err_pending_start.store(false, Ordering::Release);
            err_armed.store(false, Ordering::Release);
            err_recording.store(false, Ordering::SeqCst);
            err_rebuild.store(true, Ordering::SeqCst);
        }
    };

    let stream = match build_stream_for_sample_format(
        &device,
        sample_format,
        &stream_config,
        sample_buffer.clone(),
        preroll_buffer.clone(),
        pending_start.clone(),
        armed.clone(),
        is_recording.clone(),
        app_handle.clone(),
        capture_channels,
        capture_rate,
        downsample_ratio,
        filter_cutoff,
        || Box::new(make_err_fn()),
    ) {
        Ok(stream) => stream,
        Err(error)
            if matches!(stream_config.buffer_size, cpal::BufferSize::Fixed(_)) =>
        {
            eprintln!(
                "[FamVoice] Falling back to host default microphone buffer after low-latency request failed: {}",
                error
            );
            build_stream_for_sample_format(
                &device,
                sample_format,
                &default_stream_config,
                sample_buffer.clone(),
                preroll_buffer.clone(),
                pending_start.clone(),
                armed.clone(),
                is_recording.clone(),
                app_handle.clone(),
                capture_channels,
                capture_rate,
                downsample_ratio,
                filter_cutoff,
                || Box::new(make_err_fn()),
            )
            .map_err(|fallback_error| format!("Failed to open microphone: {}", fallback_error))?
        }
        Err(error) => {
            return Err(format!("Failed to open microphone: {}", error));
        }
    };

    Ok((stream, device_id))
}

fn build_persistent_input_stream(
    selected_device_id: Option<&str>,
    sample_buffer: Arc<Mutex<Vec<i16>>>,
    preroll_buffer: Arc<Mutex<Vec<i16>>>,
    pending_start: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    needs_rebuild: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
) -> Result<(cpal::Stream, String), String> {
    let host = cpal::default_host();
    let candidates = input_device_candidates(&host, selected_device_id)?;
    let mut last_error = None;

    for (index, device) in candidates.into_iter().enumerate() {
        match build_persistent_input_stream_for_device(
            device,
            sample_buffer.clone(),
            preroll_buffer.clone(),
            pending_start.clone(),
            armed.clone(),
            is_recording.clone(),
            needs_rebuild.clone(),
            app_handle.clone(),
        ) {
            Ok((stream, device_id)) => return Ok((stream, device_id)),
            Err(error) => {
                if index == 0 && selected_device_id.is_some() {
                    eprintln!(
                        "[FamVoice] Selected microphone failed to open, falling back to default: {}",
                        error
                    );
                }
                last_error = Some(error);
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| "No microphone found. Check your audio input device.".to_string()))
}

impl Default for AudioState {
    fn default() -> Self {
        let (tx, mut rx) = mpsc::channel::<AudioCommand>(10);
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_recording_clone = is_recording.clone();
        let pending_start = Arc::new(AtomicBool::new(false));
        let pending_start_clone = pending_start.clone();
        let armed = Arc::new(AtomicBool::new(false));
        let armed_clone = armed.clone();
        let needs_rebuild = Arc::new(AtomicBool::new(false));
        let needs_rebuild_clone = needs_rebuild.clone();
        std::thread::spawn(move || {
            let sample_buffer: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
            let preroll_buffer: Arc<Mutex<Vec<i16>>> =
                Arc::new(Mutex::new(Vec::with_capacity(PREROLL_SAMPLES)));
            let mut recording_state = RecordingCycleState {
                pending_start: false,
                armed: false,
                is_recording: false,
            };
            let mut stream: Option<cpal::Stream> = None;
            let mut active_device_id: Option<String> = None;
            let mut active_requested_device_id: Option<String> = None;

            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    AudioCommand::Prime(app_handle, selected_device_id, reply) => {
                        let normalized_selected_device_id = selected_device_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|id| !id.is_empty())
                            .map(str::to_string);

                        let requested_device_changed =
                            normalized_selected_device_id != active_requested_device_id;
                        let selected_device_not_active = normalized_selected_device_id
                            .as_deref()
                            .is_some_and(|selected_id| {
                                active_device_id.as_deref() != Some(selected_id)
                            });

                        if requested_device_changed || selected_device_not_active {
                            stream.take();
                            active_device_id = None;
                            active_requested_device_id = normalized_selected_device_id.clone();
                        }

                        if needs_rebuild_clone.swap(false, Ordering::SeqCst) {
                            stream.take();
                            active_device_id = None;
                        }

                        if stream.is_none() {
                            match build_persistent_input_stream(
                                normalized_selected_device_id.as_deref(),
                                sample_buffer.clone(),
                                preroll_buffer.clone(),
                                pending_start_clone.clone(),
                                armed_clone.clone(),
                                is_recording_clone.clone(),
                                needs_rebuild_clone.clone(),
                                app_handle.clone(),
                            ) {
                                Ok((new_stream, device_id)) => {
                                    stream = Some(new_stream);
                                    active_device_id = Some(device_id);
                                }
                                Err(error) => {
                                    let _ = reply.send(Err(error));
                                    continue;
                                }
                            }
                        }

                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Start(app_handle, selected_device_id, reply) => {
                        let normalized_selected_device_id = selected_device_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|id| !id.is_empty())
                            .map(str::to_string);

                        let requested_device_changed =
                            normalized_selected_device_id != active_requested_device_id;
                        let selected_device_not_active = normalized_selected_device_id
                            .as_deref()
                            .is_some_and(|selected_id| {
                                active_device_id.as_deref() != Some(selected_id)
                            });

                        if requested_device_changed || selected_device_not_active {
                            stream.take();
                            active_device_id = None;
                            active_requested_device_id = normalized_selected_device_id.clone();
                        }

                        if needs_rebuild_clone.swap(false, Ordering::SeqCst) {
                            stream.take();
                            active_device_id = None;
                        }

                        if stream.is_none() {
                            match build_persistent_input_stream(
                                normalized_selected_device_id.as_deref(),
                                sample_buffer.clone(),
                                preroll_buffer.clone(),
                                pending_start_clone.clone(),
                                armed_clone.clone(),
                                is_recording_clone.clone(),
                                needs_rebuild_clone.clone(),
                                app_handle.clone(),
                            ) {
                                Ok((new_stream, device_id)) => {
                                    stream = Some(new_stream);
                                    active_device_id = Some(device_id);
                                }
                                Err(error) => {
                                    let _ = reply.send(Err(error));
                                    continue;
                                }
                            }
                        }

                        {
                            let mut buffer = sample_buffer.lock().unwrap();
                            let mut preroll = preroll_buffer.lock().unwrap();
                            prepare_recording_cycle(
                                &mut recording_state,
                                &mut buffer,
                                &mut preroll,
                            );
                        }
                        pending_start_clone.store(recording_state.pending_start, Ordering::Release);
                        armed_clone.store(recording_state.armed, Ordering::Release);
                        is_recording_clone.store(recording_state.is_recording, Ordering::SeqCst);

                        if let Some(ref s) = stream {
                            if let Err(e) = s.play() {
                                eprintln!("[FamVoice] Failed to resume stream: {}", e);
                                stream.take();
                                active_device_id = None;
                                recording_state.pending_start = false;
                                recording_state.armed = false;
                                recording_state.is_recording = false;
                                pending_start_clone.store(false, Ordering::Release);
                                armed_clone.store(false, Ordering::Release);
                                is_recording_clone.store(false, Ordering::SeqCst);
                                needs_rebuild_clone.store(false, Ordering::SeqCst);
                                let _ =
                                    reply.send(Err(format!("Failed to resume microphone: {}", e)));
                                continue;
                            }
                        }

                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Stop(reply) => {
                        pending_start_clone.store(false, Ordering::Release);
                        let samples = {
                            let mut buffer = sample_buffer.lock().unwrap();
                            finish_recording_cycle(&mut recording_state, &mut buffer)
                        };
                        pending_start_clone.store(recording_state.pending_start, Ordering::Release);
                        armed_clone.store(recording_state.armed, Ordering::Release);
                        is_recording_clone.store(recording_state.is_recording, Ordering::SeqCst);

                        // Pause the stream to release the microphone between recordings.
                        if let Some(ref s) = stream {
                            if let Err(e) = s.pause() {
                                eprintln!("[FamVoice] Failed to pause stream: {}", e);
                            }
                        }

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
    #[cfg(target_endian = "little")]
    {
        let byte_len = std::mem::size_of_val(samples);
        let sample_bytes =
            unsafe { std::slice::from_raw_parts(samples.as_ptr().cast::<u8>(), byte_len) };
        buf.extend_from_slice(sample_bytes);
    }
    #[cfg(not(target_endian = "little"))]
    {
        for &sample in samples {
            buf.extend_from_slice(&sample.to_le_bytes());
        }
    }

    buf
}

/// Encode PCM samples as FLAC bytes in memory (16kHz, mono, 16-bit).
/// Typically compresses to ~30-40% of the equivalent WAV size.
pub fn encode_flac_in_memory(samples: &[i16]) -> Result<Vec<u8>, String> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    // TODO: flacenc::source::MemSource stores Vec<i32> internally and from_samples
    // accepts &[i32], so this i16->i32 widening copy is unavoidable (~3.7MB for 60s).
    // If flacenc adds an i16 source in the future, switch to avoid this allocation.
    let samples_i32: Vec<i32> = samples.iter().map(|&s| s as i32).collect();

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| format!("Invalid FLAC encoder config: {:?}", e))?;

    let source =
        flacenc::source::MemSource::from_samples(&samples_i32, 1, 16, TARGET_SAMPLE_RATE as usize);

    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| format!("FLAC encode failed: {:?}", e))?;

    let mut sink = flacenc::bitsink::ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| format!("FLAC write failed: {:?}", e))?;

    Ok(sink.as_slice().to_vec())
}

fn take_recorded_samples(buffer: &mut Vec<i16>) -> Option<Vec<i16>> {
    if buffer.is_empty() {
        return None;
    }

    Some(std::mem::take(buffer))
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

fn speech_window_end_frame(
    frame_levels: &[f64],
    threshold: f64,
    start_frame: usize,
) -> Option<usize> {
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

pub fn select_samples_for_upload<'a>(
    samples: &'a [i16],
    silence_threshold_rms: f64,
) -> UploadAudioSelection<'a> {
    if samples.len() < SPEECH_WINDOW_MIN_CLIP_SAMPLES {
        return UploadAudioSelection {
            samples: Cow::Borrowed(samples),
            was_trimmed: false,
        };
    }

    let frame_levels = speech_frame_levels(samples);
    let Some(start_frame) = speech_window_start_frame(&frame_levels, silence_threshold_rms) else {
        return UploadAudioSelection {
            samples: Cow::Borrowed(samples),
            was_trimmed: false,
        };
    };
    let Some(end_frame) =
        speech_window_end_frame(&frame_levels, silence_threshold_rms, start_frame)
    else {
        return UploadAudioSelection {
            samples: Cow::Borrowed(samples),
            was_trimmed: false,
        };
    };

    let start_sample = (start_frame * SPEECH_WINDOW_FRAME_SAMPLES)
        .saturating_sub(SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES);
    let end_sample = (((end_frame + 1) * SPEECH_WINDOW_FRAME_SAMPLES)
        + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES)
        .min(samples.len());

    if end_sample <= start_sample {
        return UploadAudioSelection {
            samples: Cow::Borrowed(samples),
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
            samples: Cow::Borrowed(samples),
            was_trimmed: false,
        };
    }

    UploadAudioSelection {
        samples: Cow::Owned(samples[start_sample..end_sample].to_vec()),
        was_trimmed: true,
    }
}

pub async fn start_recording(
    app_handle: tauri::AppHandle,
    state: &AudioState,
    selected_device_id: Option<&str>,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .cmd_tx
        .send(AudioCommand::Start(
            app_handle,
            selected_device_id.map(str::to_string),
            tx,
        ))
        .await
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|e| format!("Recording channel error: {}", e))?
}

pub async fn prime_input_stream(
    app_handle: tauri::AppHandle,
    state: &AudioState,
    selected_device_id: Option<&str>,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .cmd_tx
        .send(AudioCommand::Prime(
            app_handle,
            selected_device_id.map(str::to_string),
            tx,
        ))
        .await
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|e| format!("Prime channel error: {}", e))?
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
    fn recording_cycle_prepare_clears_stale_audio_and_marks_pending() {
        let mut samples = vec![7, 8, 9];
        let mut preroll = vec![4, 5, 6];
        let mut state = RecordingCycleState {
            pending_start: false,
            armed: false,
            is_recording: false,
        };

        prepare_recording_cycle(&mut state, &mut samples, &mut preroll);

        assert!(samples.is_empty());
        assert!(preroll.is_empty());
        assert_eq!(
            state,
            RecordingCycleState {
                pending_start: true,
                armed: false,
                is_recording: true,
            }
        );
    }

    #[test]
    fn recording_cycle_activation_moves_warmup_audio_into_recording_buffer() {
        let mut samples = Vec::new();
        let mut preroll = vec![10, 20, 30];
        let warmup = vec![40, 50];
        let mut state = RecordingCycleState {
            pending_start: true,
            armed: false,
            is_recording: false,
        };

        activate_recording_cycle(&mut state, &mut samples, &mut preroll, &warmup);

        assert_eq!(samples, vec![10, 20, 30, 40, 50]);
        assert!(preroll.is_empty());
        assert_eq!(
            state,
            RecordingCycleState {
                pending_start: false,
                armed: true,
                is_recording: true,
            }
        );
    }

    #[test]
    fn preroll_buffer_keeps_most_recent_samples() {
        let mut preroll = vec![1; PREROLL_SAMPLES - 2];

        append_preroll_samples_capped(&mut preroll, &[2, 3, 4, 5]);

        assert_eq!(preroll.len(), PREROLL_SAMPLES);
        assert_eq!(preroll[0], 1);
        assert_eq!(&preroll[preroll.len() - 4..], &[2, 3, 4, 5]);
    }

    #[test]
    fn recording_cycle_stop_disarms_capture_and_returns_buffered_samples() {
        let mut samples = vec![1, 2, 3];
        let mut state = RecordingCycleState {
            pending_start: false,
            armed: true,
            is_recording: true,
        };

        let captured = finish_recording_cycle(&mut state, &mut samples);

        assert_eq!(captured, Some(vec![1, 2, 3]));
        assert!(samples.is_empty());
        assert_eq!(
            state,
            RecordingCycleState {
                pending_start: false,
                armed: false,
                is_recording: false,
            }
        );
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

    #[test]
    fn recording_buffer_is_capped_at_five_minutes() {
        let mut samples = Vec::new();
        let chunk = vec![1; MAX_RECORDED_SAMPLES / 2];

        append_samples_capped(&mut samples, &chunk);
        append_samples_capped(&mut samples, &chunk);
        append_samples_capped(&mut samples, &[2; 128]);

        assert_eq!(samples.len(), MAX_RECORDED_SAMPLES);
        assert!(samples.iter().all(|sample| *sample == 1));
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
        assert_eq!(
            &selected.samples[..SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES],
            silence_samples(SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES).as_slice()
        );
        assert_eq!(selected.samples[SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES], 800);
    }

    #[test]
    fn speech_window_trims_trailing_silence() {
        let mut samples = speech_samples(TARGET_SAMPLE_RATE as usize, 800);
        samples.extend(silence_samples(TARGET_SAMPLE_RATE as usize));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() < samples.len());
        assert_eq!(
            &selected.samples[..SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES],
            speech_samples(SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES, 800).as_slice()
        );
        assert!(
            selected.samples.len()
                <= TARGET_SAMPLE_RATE as usize
                    + SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES
                    + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES
                    + SPEECH_WINDOW_FRAME_SAMPLES
        );
    }

    #[test]
    fn speech_window_trims_both_leading_and_trailing_silence() {
        let mut samples = silence_samples(TARGET_SAMPLE_RATE as usize);
        samples.extend(speech_samples(TARGET_SAMPLE_RATE as usize, 800));
        samples.extend(silence_samples(TARGET_SAMPLE_RATE as usize));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert!(selected.samples.len() < samples.len());
        assert_eq!(
            &selected.samples[..SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES],
            silence_samples(SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES).as_slice()
        );
        assert_eq!(selected.samples[SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES], 800);
        assert!(
            selected.samples.len()
                <= TARGET_SAMPLE_RATE as usize
                    + SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES
                    + SPEECH_WINDOW_TRAILING_CONTEXT_SAMPLES
                    + SPEECH_WINDOW_FRAME_SAMPLES
        );
    }

    #[test]
    fn speech_window_keeps_short_clips_untrimmed() {
        let mut samples = silence_samples(SPEECH_WINDOW_FRAME_SAMPLES * 2);
        samples.extend(speech_samples(SPEECH_WINDOW_FRAME_SAMPLES * 8, 800));
        samples.extend(silence_samples(SPEECH_WINDOW_FRAME_SAMPLES * 2));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(!selected.was_trimmed);
        assert_eq!(selected.samples.as_ref(), samples.as_slice());
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

    #[test]
    fn speech_window_keeps_leading_context_before_detected_speech() {
        let mut samples = silence_samples(TARGET_SAMPLE_RATE as usize);
        samples.extend(speech_samples(TARGET_SAMPLE_RATE as usize, 800));

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(selected.was_trimmed);
        assert_eq!(selected.samples.len(), TARGET_SAMPLE_RATE as usize + SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES);
        assert_eq!(
            &selected.samples[..SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES],
            silence_samples(SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES).as_slice()
        );
        assert_eq!(selected.samples[SPEECH_WINDOW_LEADING_CONTEXT_SAMPLES], 800);
    }

    #[test]
    fn speech_window_borrows_original_samples_when_no_trim_is_needed() {
        let samples = speech_samples(TARGET_SAMPLE_RATE as usize, 800);

        let selected = select_samples_for_upload(&samples, 100.0);

        assert!(!selected.was_trimmed);
        assert!(matches!(selected.samples, Cow::Borrowed(_)));
    }
}
