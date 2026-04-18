mod audio;
mod clipboard;
mod dpapi;
mod glossary;
mod history;
mod injection;
mod input_hook;
mod mic_analysis;
mod prompt_optimizer;
mod settings;
mod startup;
mod transcription;
mod window;

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::VecDeque, time::Duration};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{include_image, AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use audio::AudioState;
use clipboard::ClipboardState;
use history::HistoryState;
use settings::{AppSettings, FrontendSettings, SaveSettingsRequest, SettingsState};

const PASTE_CLIPBOARD_SETTLE_DELAY_MS: u64 = 2;
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 25;
const STATUS_RESET_DELAY_MS: u64 = 2_000;
const PROMPT_OPTIMIZER_TIMEOUT_MS: u64 = 10_000;
const MIN_RESIZE_DIMENSION: f64 = 50.0;
const MAX_RESIZE_DIMENSION: f64 = 4000.0;
const MAX_REPASTE_TEXT_BYTES: usize = 50 * 1024;

pub struct HttpClientState {
    pub client: reqwest::Client,
}

pub struct BackgroundTasksState {
    handles: std::sync::Mutex<VecDeque<tokio::task::JoinHandle<()>>>,
    status_reset_generation: std::sync::atomic::AtomicU64,
}

impl BackgroundTasksState {
    fn new() -> Self {
        Self {
            handles: std::sync::Mutex::new(VecDeque::new()),
            status_reset_generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn spawn(&self, handle: tokio::task::JoinHandle<()>) {
        if let Ok(mut handles) = self.handles.lock() {
            handles.push_back(handle);
            if handles.len() > 10 {
                if let Some(oldest_handle) = handles.pop_front() {
                    oldest_handle.abort();
                }
            }
        }
    }

    fn invalidate_status_reset(&self) {
        self.status_reset_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn schedule_status_reset_generation(&self) -> u64 {
        self.status_reset_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }

    fn is_current_status_reset_generation(&self, generation: u64) -> bool {
        self.status_reset_generation
            .load(std::sync::atomic::Ordering::SeqCst)
            == generation
    }
}

impl Drop for BackgroundTasksState {
    fn drop(&mut self) {
        if let Ok(handles) = self.handles.get_mut() {
            for handle in handles.drain(..) {
                handle.abort();
            }
        }
    }
}

use input_hook::HotkeyConfigState;
use std::sync::Mutex;

fn hotkey_is_disabled(hotkey: &str) -> bool {
    hotkey.trim().is_empty()
}

fn log_operation_error(operation: &str, error: &str) {
    eprintln!("[FamVoice] {operation}: {error}");
}

fn handle_recording_shortcut_event(app: &AppHandle, event_state: ShortcutState) {
    if event_state == ShortcutState::Pressed {
        let state: State<AudioState> = app.state();
        if state
            .is_recording
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = start_recording_cmd(app_clone.clone()).await;
            });
        }
    } else if event_state == ShortcutState::Released {
        let state: State<AudioState> = app.state();
        if state
            .is_recording
            .compare_exchange(
                true,
                false,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = stop_recording_cmd(app_clone.clone()).await;
            });
        }
    }
}

fn register_hotkeys(app: &AppHandle, recording_hotkey: &str, repaste_hotkey: &str) {
    input_hook::reset_mouse_hotkey_state();

    if let Some(state) = app.try_state::<HotkeyConfigState>() {
        if let Ok(mut guard) = state.hotkey.lock() {
            *guard = recording_hotkey.to_string();
        }
    }

    let _ = app.global_shortcut().unregister_all();

    if input_hook::is_mouse_hotkey(recording_hotkey) {
        eprintln!(
            "[FamVoice] Mouse hotkey registered globally: {}",
            recording_hotkey
        );
    } else if let Ok(shortcut) = recording_hotkey.parse::<Shortcut>() {
        let _ = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                handle_recording_shortcut_event(app, event.state());
            });
    } else {
        eprintln!("[FamVoice] Failed to parse hotkey: {}", recording_hotkey);
    }

    if hotkey_is_disabled(repaste_hotkey) {
        return;
    }

    if let Ok(shortcut) = repaste_hotkey.parse::<Shortcut>() {
        let _ = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = repaste_last_history_item(app_clone.clone()).await {
                            log_operation_error("Re-paste hotkey failed", &error);
                        }
                    });
                }
            });
    } else {
        eprintln!(
            "[FamVoice] Failed to parse re-paste hotkey: {}",
            repaste_hotkey
        );
    }
}

async fn paste_text_via_clipboard(
    app: &AppHandle,
    text: &str,
    preserve_clipboard: bool,
) -> Result<(), String> {
    if text.len() > MAX_REPASTE_TEXT_BYTES {
        return Err("History item is too large to repaste".to_string());
    }

    let clipboard_state: State<ClipboardState> = app.state();

    if preserve_clipboard {
        clipboard::save_clipboard(&clipboard_state);
    }

    clipboard::set_clipboard(&clipboard_state, text)
        .map_err(|error| format!("Failed to set clipboard: {}", error))?;

    tokio::time::sleep(paste_clipboard_settle_delay()).await;
    let paste_result = tokio::task::spawn_blocking(injection::simulate_paste)
        .await
        .map_err(|error| format!("Paste task panicked: {}", error))?;

    if preserve_clipboard {
        let saved_clipboard = clipboard::saved_clipboard_text(&clipboard_state);
        let tasks_state: State<BackgroundTasksState> = app.state();
        let app_handle = app.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(clipboard_restore_delay()).await;
            if let Some(saved_text) = saved_clipboard {
                let clipboard_state: State<ClipboardState> = app_handle.state();
                if let Err(error) = clipboard::restore_clipboard_text(&clipboard_state, &saved_text)
                {
                    log_operation_error("Failed to restore clipboard after repaste", &error);
                }
            }
        });
        tasks_state.spawn(handle);
    }

    paste_result.map_err(|error| format!("Failed to simulate paste: {}", error))
}

fn latest_history_text(history_state: &HistoryState) -> Result<String, String> {
    let items = history_state
        .items
        .lock()
        .map_err(|e| format!("Failed to acquire history lock: {}", e))?;
    items
        .first()
        .map(|item| item.text.clone())
        .ok_or_else(|| "No history item available to re-paste".to_string())
}

async fn repaste_last_history_item(app: AppHandle) -> Result<(), String> {
    let history_state: State<HistoryState> = app.state();
    let text = latest_history_text(&history_state)?;
    let settings_state: State<SettingsState> = app.state();
    let preserve_clipboard = settings_state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .preserve_clipboard;

    paste_text_via_clipboard(&app, &text, preserve_clipboard).await
}

fn normalize_frontend_settings(settings: &AppSettings) -> FrontendSettings {
    let mut frontend = settings.to_frontend();

    if !frontend.input_device_id.is_empty() {
        match audio::list_input_devices() {
            Ok(devices) => {
                if !devices
                    .iter()
                    .any(|device| device.id == frontend.input_device_id)
                {
                    frontend.input_device_id.clear();
                }
            }
            Err(error) => {
                eprintln!(
                    "[FamVoice] Failed to validate selected microphone: {}",
                    error
                );
                frontend.input_device_id.clear();
            }
        }
    }

    frontend
}

#[tauri::command]
fn get_settings(state: State<'_, SettingsState>) -> Result<FrontendSettings, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;
    Ok(normalize_frontend_settings(&settings))
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
    new_settings: SaveSettingsRequest,
) -> Result<FrontendSettings, String> {
    let previous = state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .clone();
    let saved = state.save_request(new_settings)?;
    let frontend = normalize_frontend_settings(&saved);

    if previous.widget_mode != saved.widget_mode {
        window::apply_main_window_mode(&app, saved.widget_mode, true)?;
    }

    if previous.hotkey != saved.hotkey || previous.repaste_hotkey != saved.repaste_hotkey {
        register_hotkeys(&app, &saved.hotkey, &saved.repaste_hotkey);
    }

    if previous.input_device_id != saved.input_device_id {
        let audio_state = {
            let state: State<AudioState> = app.state();
            (*state).clone()
        };
        if let Err(error) = audio::prime_input_stream(
            app.clone(),
            &audio_state,
            Some(saved.input_device_id.as_str()),
        )
        .await
        {
            eprintln!("[FamVoice] Failed to prime selected microphone: {}", error);
        }
    }

    let _ = app.emit("settings-updated", frontend.clone());

    Ok(frontend)
}

fn sanitize_window_dimension(value: f64, label: &str) -> Result<f64, String> {
    if !value.is_finite() {
        return Err(format!("{label} must be finite"));
    }

    Ok(value.clamp(MIN_RESIZE_DIMENSION, MAX_RESIZE_DIMENSION))
}

#[tauri::command]
fn resize_main_window(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let width = sanitize_window_dimension(width, "width")?;
    let height = sanitize_window_dimension(height, "height")?;
    window::resize_main_window(&app, width, height)
}

#[tauri::command]
fn get_history(state: State<'_, HistoryState>) -> Result<Vec<history::HistoryItem>, String> {
    let items = state
        .items
        .lock()
        .map_err(|e| format!("Failed to acquire history lock: {}", e))?;
    Ok(items.clone())
}

#[tauri::command]
fn delete_history_item(app: AppHandle, state: State<'_, HistoryState>, id: u64) {
    state.delete(id);
    emit_history_updated(&app, &state);
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<'_, HistoryState>) {
    state.clear();
    emit_history_updated(&app, &state);
}

#[tauri::command]
async fn repaste_history_item(app: AppHandle, text: String) -> Result<(), String> {
    let settings_state: State<SettingsState> = app.state();
    let preserve_clipboard = settings_state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .preserve_clipboard;

    paste_text_via_clipboard(&app, &text, preserve_clipboard).await
}

#[tauri::command]
fn list_input_devices() -> Result<Vec<audio::InputDeviceOption>, String> {
    audio::list_input_devices()
}

#[tauri::command]
async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    window::close_settings_window(&app);
    Ok(())
}

#[tauri::command]
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    window::open_settings_window(&app)
}

#[tauri::command]
fn can_manage_autostart() -> bool {
    startup::current_executable_supports_autostart()
}

fn transcription_language_override(language_preference: &str) -> Option<&str> {
    let trimmed = language_preference.trim();
    match trimmed {
        "" | "auto" => None,
        _ => Some(trimmed),
    }
}

#[tauri::command]
async fn start_recording_cmd(app: AppHandle) -> Result<(), String> {
    let audio_state: State<AudioState> = app.state();
    let tasks_state: State<BackgroundTasksState> = app.state();
    let settings_state: State<SettingsState> = app.state();
    tasks_state.invalidate_status_reset();
    let input_device_id = settings_state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .input_device_id
        .clone();

    match audio::start_recording(app.clone(), &audio_state, Some(input_device_id.as_str())).await {
        Ok(()) => {
            let _ = app.emit("status", "recording");
            Ok(())
        }
        Err(error) => {
            eprintln!("[FamVoice] Failed to start recording: {}", error);
            let _ = app.emit("status", "error");
            let _ = app.emit("transcript", error.clone());
            Err(error)
        }
    }
}

fn prompt_optimizer_timeout(model: &str) -> std::time::Duration {
    let _ = model;
    std::time::Duration::from_millis(PROMPT_OPTIMIZER_TIMEOUT_MS)
}

fn prompt_optimizer_timeout_message(model: &str, timeout_duration: std::time::Duration) -> String {
    format!(
        "[FamVoice] Prompt optimization timed out for model {} after {}ms, using finalized transcript",
        model,
        timeout_duration.as_millis()
    )
}

fn prompt_optimizer_start_message(model: &str) -> String {
    format!(
        "[FamVoice] Starting prompt optimization with model {}",
        model
    )
}

fn prompt_optimizer_success_message(model: &str, elapsed: std::time::Duration) -> String {
    format!(
        "[FamVoice] Prompt optimization succeeded with model {} in {}ms",
        model,
        elapsed.as_millis()
    )
}

fn prompt_optimizer_failure_message(model: &str, error: &str) -> String {
    format!(
        "[FamVoice] Prompt optimization failed for model {}, using finalized transcript: {}",
        model, error
    )
}

async fn resolve_final_output_for_paste<Optimize, OptimizeFuture>(
    settings: &AppSettings,
    finalized_transcript: String,
    timeout_duration: std::time::Duration,
    optimize: Optimize,
) -> String
where
    Optimize: FnOnce(prompt_optimizer::PromptOptimizerRequest) -> OptimizeFuture,
    OptimizeFuture: Future<
        Output = Result<
            prompt_optimizer::PromptOptimizerResponse,
            prompt_optimizer::PromptOptimizerError,
        >,
    >,
{
    if !settings.prompt_optimization_enabled {
        return finalized_transcript;
    }

    let api_key = settings.api_key.trim();
    if api_key.is_empty() {
        return finalized_transcript;
    }

    let request = prompt_optimizer::PromptOptimizerRequest {
        model: settings.prompt_optimizer_model.clone(),
        source_transcript: finalized_transcript.clone(),
    };

    eprintln!(
        "{}",
        prompt_optimizer_start_message(&settings.prompt_optimizer_model)
    );
    let optimization_started_at = std::time::Instant::now();

    match tokio::time::timeout(timeout_duration, optimize(request)).await {
        Ok(Ok(response)) => {
            eprintln!(
                "{}",
                prompt_optimizer_success_message(
                    &settings.prompt_optimizer_model,
                    optimization_started_at.elapsed()
                )
            );
            response.optimized_prompt
        }
        Ok(Err(error)) => {
            eprintln!(
                "{}",
                prompt_optimizer_failure_message(
                    &settings.prompt_optimizer_model,
                    &error.to_string()
                )
            );
            finalized_transcript
        }
        Err(_) => {
            eprintln!(
                "{}",
                prompt_optimizer_timeout_message(
                    &settings.prompt_optimizer_model,
                    timeout_duration
                )
            );
            finalized_transcript
        }
    }
}

#[cfg(test)]
fn should_restore_clipboard(
    auto_paste: bool,
    preserve_clipboard: bool,
    paste_successful: bool,
) -> bool {
    auto_paste && preserve_clipboard && paste_successful
}

#[cfg(test)]
fn should_store_history(auto_paste: bool, paste_successful: bool) -> bool {
    auto_paste && paste_successful
}

fn paste_clipboard_settle_delay() -> std::time::Duration {
    std::time::Duration::from_millis(PASTE_CLIPBOARD_SETTLE_DELAY_MS)
}

fn clipboard_restore_delay() -> std::time::Duration {
    std::time::Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS)
}

fn status_reset_delay() -> std::time::Duration {
    std::time::Duration::from_millis(STATUS_RESET_DELAY_MS)
}

fn emit_history_updated(app: &AppHandle, history_state: &HistoryState) {
    let items = {
        let guard = match history_state.items.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.clone()
    };
    let _ = app.emit("history-updated", &items);
}

fn schedule_status_reset(app: AppHandle, tasks_state: &BackgroundTasksState) {
    let generation = tasks_state.schedule_status_reset_generation();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(status_reset_delay()).await;
        let tasks_state: State<BackgroundTasksState> = app.state();
        if tasks_state.is_current_status_reset_generation(generation) {
            let _ = app.emit("status", "idle");
        }
    });
    tasks_state.spawn(handle);
}

fn emit_transient_recording_error(
    app: &AppHandle,
    tasks_state: &BackgroundTasksState,
    message: &str,
) {
    let _ = app.emit("status", "error");
    let _ = app.emit("transcript", message);
    schedule_status_reset(app.clone(), tasks_state);
}

struct PreparedRecording {
    settings: AppSettings,
    samples: Vec<i16>,
    silence_threshold: f64,
}

async fn capture_and_prepare_samples(
    app: &AppHandle,
    tasks_state: &BackgroundTasksState,
    audio_state: &AudioState,
    settings_state: &SettingsState,
) -> Result<PreparedRecording, String> {
    let mut samples = match audio::stop_recording(audio_state).await {
        Some(samples) => samples,
        None => {
            eprintln!("[FamVoice] stop_recording returned None, was not recording");
            let _ = app.emit("status", "idle");
            return Err("Not recording".into());
        }
    };

    if samples.is_empty() {
        eprintln!("[FamVoice] No audio samples recorded");
        emit_transient_recording_error(app, tasks_state, "No audio recorded");
        return Err("No audio recorded".into());
    }

    let settings = settings_state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .clone();
    let levels = mic_analysis::analyze(&samples);
    let silence_threshold = mic_analysis::silence_threshold(settings.mic_sensitivity);
    let level_details = mic_analysis::level_details(levels);
    let silence_threshold_dbfs = mic_analysis::dbfs(silence_threshold);
    eprintln!(
        "[FamVoice] Audio levels: rms {:.2} ({:.1} dBFS), peak {:.0} ({:.1}%), silence threshold {:.2} ({:.1} dBFS), sensitivity {}",
        levels.rms,
        level_details.rms_dbfs,
        levels.peak,
        level_details.peak_percent,
        silence_threshold,
        silence_threshold_dbfs,
        settings.mic_sensitivity
    );

    if mic_analysis::should_reject_for_silence(levels, settings.mic_sensitivity) {
        eprintln!("[FamVoice] Silence detected, skipping transcription");
        emit_transient_recording_error(app, tasks_state, "No voice detected");
        return Err("No voice detected".into());
    }

    if let Some(gain) = mic_analysis::normalize_quiet_audio(&mut samples, settings.mic_sensitivity)
    {
        let boosted_levels = mic_analysis::analyze(&samples);
        let boosted_details = mic_analysis::level_details(boosted_levels);
        eprintln!(
            "[FamVoice] Applied mic gain {:.2}x -> rms {:.2} ({:.1} dBFS), peak {:.0} ({:.1}%)",
            gain,
            boosted_levels.rms,
            boosted_details.rms_dbfs,
            boosted_levels.peak,
            boosted_details.peak_percent
        );
    }

    match audio::maybe_apply_noise_suppression(&mut samples, settings.noise_suppression_enabled) {
        Ok(true) => {
            let denoised_levels = mic_analysis::analyze(&samples);
            let denoised_details = mic_analysis::level_details(denoised_levels);
            eprintln!(
                "[FamVoice] Applied noise suppression -> rms {:.2} ({:.1} dBFS), peak {:.0} ({:.1}%)",
                denoised_levels.rms,
                denoised_details.rms_dbfs,
                denoised_levels.peak,
                denoised_details.peak_percent
            );
        }
        Ok(false) => {}
        Err(error) => {
            eprintln!("[FamVoice] Noise suppression skipped: {}", error);
        }
    }

    if settings.transcription_api_key().is_empty() {
        let provider_label = if settings.transcription_provider == "groq" {
            "Groq"
        } else {
            "OpenAI"
        };
        eprintln!("[FamVoice] {} API key is empty!", provider_label);
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", format!("{} API key missing", provider_label));
        return Err("API key is empty".into());
    }

    Ok(PreparedRecording {
        settings,
        samples,
        silence_threshold,
    })
}

async fn transcribe_recording(
    http_client: &reqwest::Client,
    settings: &AppSettings,
    samples: Vec<i16>,
    silence_threshold: f64,
    started_at: std::time::Instant,
) -> Result<String, String> {
    let t_encode = std::time::Instant::now();
    let upload_audio = audio::select_samples_for_upload(&samples, silence_threshold);
    let sample_rate = 16_000.0;

    if upload_audio.was_trimmed {
        eprintln!(
            "[FamVoice] Speech window trimmed upload from {} samples ({:.1}s) to {} samples ({:.1}s)",
            samples.len(),
            samples.len() as f64 / sample_rate,
            upload_audio.samples.len(),
            upload_audio.samples.len() as f64 / sample_rate
        );
    }

    let (audio_bytes, audio_mime, audio_ext, format_label) =
        match audio::encode_flac_in_memory(upload_audio.samples.as_ref()) {
            Ok(flac_bytes) => (flac_bytes, "audio/flac", "audio.flac", "FLAC"),
            Err(flac_err) => {
                eprintln!(
                    "[FamVoice] FLAC encode failed, falling back to WAV: {}",
                    flac_err
                );
                let wav = audio::encode_wav_in_memory(upload_audio.samples.as_ref());
                (wav, "audio/wav", "audio.wav", "WAV")
            }
        };
    let t_api = std::time::Instant::now();
    eprintln!(
        "[FamVoice] {} encode: {} samples ({:.1}s) -> {} bytes in {:.0}ms",
        format_label,
        upload_audio.samples.len(),
        upload_audio.samples.len() as f64 / sample_rate,
        audio_bytes.len(),
        t_encode.elapsed().as_secs_f64() * 1000.0
    );

    eprintln!(
        "[FamVoice] Transcribing with provider: {}, model: {}, language preference: {}, path: upload",
        settings.transcription_provider, settings.model, settings.language
    );
    let lang = transcription_language_override(&settings.language);
    let transcription_prompt = glossary::transcription_prompt(&settings.language);
    let text = transcription::transcribe_audio(
        http_client,
        audio_bytes,
        settings.transcription_api_key(),
        &settings.model,
        lang,
        transcription_prompt.as_deref(),
        &settings.transcription_provider,
        audio_mime,
        audio_ext,
    )
    .await?;

    let finalized_text = glossary::finalize_transcript(text, &settings.replacements);
    let text = resolve_final_output_for_paste(
        settings,
        finalized_text,
        prompt_optimizer_timeout(&settings.prompt_optimizer_model),
        |request| prompt_optimizer::optimize_prompt(http_client, settings.api_key.trim(), request),
    )
    .await;
    eprintln!(
        "[FamVoice] Transcript ready: path=upload | API {:.0}ms | Total {:.0}ms | {} chars",
        t_api.elapsed().as_secs_f64() * 1000.0,
        started_at.elapsed().as_secs_f64() * 1000.0,
        text.len(),
    );
    #[cfg(debug_assertions)]
    {
        let preview = if text.len() > 100 {
            &text[..100]
        } else {
            &text
        };
        eprintln!("[FamVoice] Transcript preview: {:?}", preview);
    }

    Ok(text)
}

async fn deliver_transcript(
    app: &AppHandle,
    tasks_state: &BackgroundTasksState,
    history_state: &HistoryState,
    clipboard_state: &ClipboardState,
    settings: &AppSettings,
    text: String,
) {
    if settings.auto_paste && settings.preserve_clipboard {
        clipboard::save_clipboard(clipboard_state);
    }

    if let Err(error) = clipboard::set_clipboard(clipboard_state, &text) {
        eprintln!("[FamVoice] Failed to set clipboard: {}", error);
    }

    let mut paste_successful = true;
    let mut paste_error = None;

    if settings.auto_paste {
        tokio::time::sleep(paste_clipboard_settle_delay()).await;
        match tokio::task::spawn_blocking(injection::simulate_paste).await {
            Ok(Err(error)) => {
                eprintln!("[FamVoice] Failed to simulate paste: {}", error);
                paste_successful = false;
                paste_error = Some(error);
            }
            Err(join_error) => {
                let error = format!("Paste task panicked: {}", join_error);
                eprintln!("[FamVoice] {}", error);
                paste_successful = false;
                paste_error = Some(error);
            }
            Ok(Ok(())) => {}
        }
    }

    if settings.auto_paste && settings.preserve_clipboard {
        let saved_clipboard = clipboard::saved_clipboard_text(clipboard_state);
        let app_handle = app.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(clipboard_restore_delay()).await;
            if let Some(text) = saved_clipboard {
                let clipboard_state: State<ClipboardState> = app_handle.state();
                if let Err(error) = clipboard::restore_clipboard_text(&clipboard_state, &text) {
                    log_operation_error("Failed to restore clipboard", &error);
                }
            }
        });
        tasks_state.spawn(handle);
    }

    history_state.add(text.clone());
    emit_history_updated(app, history_state);

    if !paste_successful {
        let _ = app.emit("status", "error");
        let error_msg = format!(
            "Paste failed: {}. Transcript is on clipboard.",
            paste_error.unwrap_or_default()
        );
        let _ = app.emit("transcript", error_msg);
        return;
    }

    let _ = app.emit("transcript", text);
    let _ = app.emit("status", "success");
}

#[tauri::command]
async fn stop_recording_cmd(app: AppHandle) -> Result<(), String> {
    let audio_state: State<AudioState> = app.state();
    let tasks_state: State<BackgroundTasksState> = app.state();
    tasks_state.invalidate_status_reset();
    let _ = app.emit("status", "transcribing");
    let settings_state: State<SettingsState> = app.state();
    let history_state: State<HistoryState> = app.state();
    let clipboard_state: State<ClipboardState> = app.state();
    let http_state: State<HttpClientState> = app.state();
    let started_at = std::time::Instant::now();

    let prepared =
        capture_and_prepare_samples(&app, &tasks_state, &audio_state, &settings_state).await?;
    let text = match transcribe_recording(
        &http_state.client,
        &prepared.settings,
        prepared.samples,
        prepared.silence_threshold,
        started_at,
    )
    .await
    {
        Ok(text) => text,
        Err(error) => {
            eprintln!("[FamVoice] Transcription error: {}", error);
            let _ = app.emit("status", "error");
            let _ = app.emit("transcript", error.clone());
            return Err(error);
        }
    };

    deliver_transcript(
        &app,
        &tasks_state,
        &history_state,
        &clipboard_state,
        &prepared.settings,
        text,
    )
    .await;
    schedule_status_reset(app.clone(), &tasks_state);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn test_transcription_language_override_keeps_preference_modes_unset() {
        assert_eq!(transcription_language_override("auto"), None);
        assert_eq!(transcription_language_override("pt"), Some("pt"));
        assert_eq!(transcription_language_override("en"), Some("en"));
        assert_eq!(transcription_language_override("fr"), Some("fr"));
    }

    #[test]
    fn test_should_restore_clipboard_only_after_successful_auto_paste_when_enabled() {
        assert!(should_restore_clipboard(true, true, true));
        assert!(!should_restore_clipboard(false, true, true));
        assert!(!should_restore_clipboard(true, false, true));
        assert!(!should_restore_clipboard(true, true, false));
    }

    #[test]
    fn test_should_store_history_only_for_successful_auto_paste() {
        assert!(should_store_history(true, true));
        assert!(!should_store_history(false, true));
        assert!(!should_store_history(true, false));
        assert!(!should_store_history(false, false));
    }

    #[test]
    fn test_release_to_paste_path_uses_short_clipboard_settle_delay() {
        assert_eq!(
            paste_clipboard_settle_delay(),
            std::time::Duration::from_millis(2)
        );
    }

    #[test]
    fn test_clipboard_restore_happens_after_short_background_delay() {
        assert_eq!(
            clipboard_restore_delay(),
            std::time::Duration::from_millis(25)
        );
    }

    #[test]
    fn test_status_reset_generation_is_current_when_scheduled() {
        let tasks = BackgroundTasksState::new();

        let generation = tasks.schedule_status_reset_generation();

        assert!(tasks.is_current_status_reset_generation(generation));
    }

    #[test]
    fn test_status_reset_generation_is_invalidated_by_new_activity() {
        let tasks = BackgroundTasksState::new();
        let generation = tasks.schedule_status_reset_generation();

        tasks.invalidate_status_reset();

        assert!(!tasks.is_current_status_reset_generation(generation));
    }

    #[tokio::test]
    async fn test_resolve_final_output_returns_finalized_transcript_when_optimization_disabled() {
        let settings = AppSettings {
            prompt_optimization_enabled: false,
            ..AppSettings::default()
        };

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(5),
            |_request| async move {
                panic!("optimizer should not be called when disabled");
            },
        )
        .await;

        assert_eq!(output, "final transcript");
    }

    #[tokio::test]
    async fn test_resolve_final_output_uses_optimized_output_on_success() {
        let settings = AppSettings {
            prompt_optimization_enabled: true,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            api_key: "sk-openai-test".to_string(),
            ..AppSettings::default()
        };

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(50),
            |request| async move {
                assert_eq!(request.model, "gpt-5.4-mini");
                assert_eq!(request.source_transcript, "final transcript");

                Ok(prompt_optimizer::PromptOptimizerResponse {
                    optimized_prompt: "optimized prompt".to_string(),
                })
            },
        )
        .await;

        assert_eq!(output, "optimized prompt");
    }

    #[tokio::test]
    async fn test_resolve_final_output_falls_back_when_optimizer_fails() {
        let settings = AppSettings {
            prompt_optimization_enabled: true,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            api_key: "sk-openai-test".to_string(),
            ..AppSettings::default()
        };

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(50),
            |_request| async move {
                Err(prompt_optimizer::PromptOptimizerError::Http(
                    "request failed".to_string(),
                ))
            },
        )
        .await;

        assert_eq!(output, "final transcript");
    }

    #[tokio::test]
    async fn test_resolve_final_output_skips_optimizer_when_openai_key_is_blank() {
        let settings = AppSettings {
            prompt_optimization_enabled: true,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            api_key: "   ".to_string(),
            ..AppSettings::default()
        };
        let call_count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&call_count);

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(50),
            move |_request| {
                calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    Ok(prompt_optimizer::PromptOptimizerResponse {
                        optimized_prompt: "optimized prompt".to_string(),
                    })
                }
            },
        )
        .await;

        assert_eq!(output, "final transcript");
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_resolve_final_output_falls_back_when_optimizer_times_out() {
        let settings = AppSettings {
            prompt_optimization_enabled: true,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            api_key: "sk-openai-test".to_string(),
            ..AppSettings::default()
        };

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(10),
            |_request| async move {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Ok(prompt_optimizer::PromptOptimizerResponse {
                    optimized_prompt: "optimized prompt".to_string(),
                })
            },
        )
        .await;

        assert_eq!(output, "final transcript");
    }

    #[test]
    fn test_prompt_optimizer_timeout_keeps_gpt_5_4_mini_fast() {
        assert_eq!(prompt_optimizer_timeout("gpt-5.4-mini").as_millis(), 10_000);
    }

    #[test]
    fn test_prompt_optimizer_timeout_keeps_default_budget_for_unknown_models() {
        assert_eq!(
            prompt_optimizer_timeout("unsupported-model").as_millis(),
            10_000
        );
    }

    #[test]
    fn test_prompt_optimizer_timeout_message_includes_model_name() {
        let message = prompt_optimizer_timeout_message(
            "gpt-5.4-mini",
            std::time::Duration::from_millis(10_000),
        );

        assert!(message.contains("gpt-5.4-mini"));
        assert!(message.contains("10000ms"));
        assert!(message.contains("using finalized transcript"));
    }

    #[test]
    fn test_prompt_optimizer_start_message_includes_model_name() {
        let message = prompt_optimizer_start_message("gpt-5.4-mini");

        assert!(message.contains("gpt-5.4-mini"));
        assert!(message.contains("Starting prompt optimization"));
    }

    #[test]
    fn test_prompt_optimizer_success_message_includes_model_name_and_duration() {
        let message = prompt_optimizer_success_message(
            "gpt-5.4-mini",
            std::time::Duration::from_millis(1842),
        );

        assert!(message.contains("gpt-5.4-mini"));
        assert!(message.contains("1842ms"));
        assert!(message.contains("succeeded"));
    }

    #[test]
    fn test_prompt_optimizer_failure_message_includes_model_name_and_error() {
        let message = prompt_optimizer_failure_message("gpt-5.4-mini", "request failed");

        assert!(message.contains("gpt-5.4-mini"));
        assert!(message.contains("request failed"));
        assert!(message.contains("using finalized transcript"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--minimized"])
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            startup::disable_unsafe_autostart_entry(app.handle());

            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&app_dir).unwrap_or_default();

            app.manage(AudioState::default());
            app.manage(SettingsState::load(app_dir.clone()));
            app.manage(HistoryState::load(app_dir));
            app.manage(ClipboardState::default());
            app.manage(BackgroundTasksState::new());
            let http_client = reqwest::Client::builder()
                .pool_max_idle_per_host(2)
                .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
                .build()
                .expect("Failed to create HTTP client");

            let warmup_client = http_client.clone();
            let warmup_provider = {
                let state: State<SettingsState> = app.state();
                let settings = state
                    .settings
                    .lock()
                    .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;
                settings.transcription_provider.clone()
            };
            tauri::async_runtime::spawn(async move {
                let endpoint = transcription::warmup_endpoint(&warmup_provider);
                let _ = warmup_client
                    .head(endpoint)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await;
                eprintln!(
                    "[FamVoice] HTTPS connection to {} pre-warmed",
                    warmup_provider
                );
            });

            app.manage(HttpClientState {
                client: http_client,
            });

            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .tooltip("FamVoice")
                .icon(include_image!("./icons/tray-icon-amber.png"))
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } => {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("highlight-widget", ());
                    }
                    _ => {}
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = open_settings_window(app_handle).await;
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            let hotkey_shared = Arc::new(Mutex::new(String::new()));
            app.manage(HotkeyConfigState {
                hotkey: hotkey_shared.clone(),
            });

            let (hotkey, repaste_hotkey, widget_mode) = {
                let state: State<SettingsState> = app.state();
                let settings = state
                    .settings
                    .lock()
                    .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;
                (
                    settings.hotkey.clone(),
                    settings.repaste_hotkey.clone(),
                    settings.widget_mode,
                )
            };
            let input_device_id = {
                let state: State<SettingsState> = app.state();
                let settings = state
                    .settings
                    .lock()
                    .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;
                settings.input_device_id.clone()
            };

            register_hotkeys(app.handle(), &hotkey, &repaste_hotkey);
            input_hook::start_mouse_listener(app.handle().clone(), hotkey_shared);
            window::apply_main_window_mode(app.handle(), widget_mode, false)?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let audio_state = {
                    let state: State<AudioState> = app_handle.state();
                    (*state).clone()
                };
                if let Err(error) = audio::prime_input_stream(
                    app_handle.clone(),
                    &audio_state,
                    Some(input_device_id.as_str()),
                )
                .await
                {
                    log_operation_error("Failed to prime microphone on startup", &error);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_input_devices,
            get_history,
            delete_history_item,
            clear_history,
            repaste_history_item,
            start_recording_cmd,
            stop_recording_cmd,
            resize_main_window,
            open_settings_window,
            close_settings_window,
            can_manage_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
