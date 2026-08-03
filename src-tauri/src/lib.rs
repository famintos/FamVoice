#[cfg(not(target_os = "windows"))]
compile_error!(
    "FamVoice is officially supported only on Windows because local history and credential recovery rely on Windows DPAPI."
);

mod audio;
mod clipboard;
mod delivery;
mod diagnostics;
mod dictation;
mod dpapi;
mod glossary;
mod history;
mod injection;
mod input_hook;
mod mic_analysis;
mod persistence;
mod prompt_optimizer;
mod retry_audio;
mod settings;
mod startup;
mod transcription;
mod user_export;
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
use dictation::{DictationActivity, DictationCoordinatorState, SessionId};
use history::HistoryState;
use settings::{AppSettings, FrontendSettings, SaveSettingsRequest, SettingsState};

const PASTE_CLIPBOARD_SETTLE_DELAY_MS: u64 = 2;
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 25;
const STATUS_RESET_DELAY_MS: u64 = 2_000;
const PROMPT_OPTIMIZER_TIMEOUT_MS: u64 = 10_000;
const MIN_RESIZE_DIMENSION: f64 = 50.0;
const MAX_RESIZE_DIMENSION: f64 = 4000.0;

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

fn ensure_main_window_visible(app: &AppHandle, focus: bool) -> Result<(), String> {
    let widget_mode = {
        let state: State<SettingsState> = app.state();
        let settings = state
            .settings
            .lock()
            .map_err(|e| format!("Failed to acquire settings lock: {e}"))?;
        settings.widget_mode
    };

    window::ensure_main_window_visible(app, widget_mode, focus)
}

fn handle_recording_shortcut_event(app: &AppHandle, event_state: ShortcutState) {
    let coordinator: State<DictationCoordinatorState> = app.state();
    if event_state == ShortcutState::Pressed {
        if coordinator.mark_hotkey_pressed() {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = start_recording_cmd(app_clone.clone()).await;
            });
        }
    } else if event_state == ShortcutState::Released && coordinator.mark_hotkey_released() {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = stop_recording_cmd(app_clone.clone()).await;
        });
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

async fn paste_text_via_clipboard(app: &AppHandle, text: &str) -> Result<(), String> {
    delivery::validate_text_length(text)?;

    let clipboard_state: State<ClipboardState> = app.state();
    clipboard::run_temporary_text_transaction(
        clipboard_state.transaction_lock(),
        text,
        paste_clipboard_settle_delay(),
        clipboard_restore_delay(),
        || clipboard::read_clipboard_text(&clipboard_state),
        |value| clipboard::set_clipboard(&clipboard_state, value),
        || async {
            tokio::task::spawn_blocking(injection::simulate_paste)
                .await
                .map_err(|error| format!("Paste task panicked: {error}"))?
                .map_err(|error| format!("Failed to simulate paste: {error}"))
        },
    )
    .await
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

    paste_text_via_clipboard(&app, &text).await
}

fn normalize_frontend_settings(state: &SettingsState, settings: &AppSettings) -> FrontendSettings {
    let mut frontend = settings.to_frontend();
    state.apply_credential_state(&mut frontend);

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
    Ok(normalize_frontend_settings(&state, &settings))
}

#[tauri::command]
fn get_dictation_activity(state: State<'_, DictationCoordinatorState>) -> DictationActivity {
    state.activity()
}

fn ensure_input_device_change_allowed(
    previous_device_id: &str,
    requested_device_id: &str,
    activity: DictationActivity,
) -> Result<(), String> {
    let requested_device_id = settings::normalize_input_device_id(requested_device_id);
    if previous_device_id != requested_device_id && activity.active {
        return Err(
            "Microphone cannot be changed while a recording or transcription is active. Try again when the current dictation finishes."
                .to_string(),
        );
    }

    Ok(())
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
    new_settings: SaveSettingsRequest,
) -> Result<FrontendSettings, String> {
    let coordinator: State<DictationCoordinatorState> = app.state();
    let _operation_guard = coordinator.lock_operation().await;
    let previous = state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .clone();
    ensure_input_device_change_allowed(
        &previous.input_device_id,
        &new_settings.input_device_id,
        coordinator.activity(),
    )?;
    let saved = state.save_request(new_settings)?;
    let frontend = normalize_frontend_settings(&state, &saved);

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
fn delete_history_item(
    app: AppHandle,
    state: State<'_, HistoryState>,
    id: u64,
) -> Result<history::HistoryItem, String> {
    let deleted_item = state.delete(id)?;
    emit_history_updated(&app, &state);
    Ok(deleted_item)
}

#[tauri::command]
fn restore_history_item(
    app: AppHandle,
    state: State<'_, HistoryState>,
    item: history::HistoryItem,
) -> Result<(), String> {
    state.restore(item)?;
    emit_history_updated(&app, &state);
    Ok(())
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<'_, HistoryState>) -> Result<(), String> {
    state.clear()?;
    emit_history_updated(&app, &state);
    Ok(())
}

#[tauri::command]
fn get_history_retention(state: State<'_, HistoryState>) -> history::HistoryRetentionPolicy {
    state.retention_policy()
}

#[tauri::command]
fn set_history_retention(
    state: State<'_, HistoryState>,
    max_items: usize,
) -> Result<history::HistoryRetentionPolicy, String> {
    state.set_max_items(max_items)?;
    Ok(state.retention_policy())
}

#[tauri::command]
fn toggle_history_pin(
    app: AppHandle,
    state: State<'_, HistoryState>,
    id: u64,
) -> Result<bool, String> {
    let pinned = state.toggle_pin(id)?;
    emit_history_updated(&app, &state);
    Ok(pinned)
}

#[tauri::command]
fn export_history(
    app: AppHandle,
    state: State<'_, HistoryState>,
    format: history::HistoryExportFormat,
) -> Result<String, String> {
    let export = state.prepare_export(format)?;
    let extension = match format {
        history::HistoryExportFormat::Txt => "txt",
        history::HistoryExportFormat::Markdown => "md",
        history::HistoryExportFormat::Json => "json",
    };
    let path = user_export::write_download(
        &app,
        "famvoice-history",
        extension,
        export.contents.as_bytes(),
    )?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
async fn repaste_history_item(app: AppHandle, text: String) -> Result<(), String> {
    paste_text_via_clipboard(&app, &text).await
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

fn shortcut_is_registered(app: &AppHandle, value: &str) -> bool {
    if input_hook::is_mouse_hotkey(value) {
        return input_hook::mouse_listener_available();
    }
    value
        .parse::<Shortcut>()
        .is_ok_and(|shortcut| app.global_shortcut().is_registered(shortcut))
}

fn diagnostics_snapshot_for_app(
    app: &AppHandle,
    settings_state: &SettingsState,
    audio_state: &AudioState,
    diagnostics_state: &diagnostics::DiagnosticsState,
) -> Result<diagnostics::DiagnosticsSnapshot, String> {
    let settings = settings_state
        .settings
        .lock()
        .map_err(|error| format!("Failed to acquire settings lock: {error}"))?
        .clone();
    let devices = audio::list_input_devices().unwrap_or_default();
    let recording_registered = shortcut_is_registered(app, &settings.hotkey);
    let repaste_registered = hotkey_is_disabled(&settings.repaste_hotkey)
        || shortcut_is_registered(app, &settings.repaste_hotkey);

    Ok(diagnostics::diagnostics_snapshot(
        &settings,
        &devices,
        audio_state
            .stream_healthy
            .load(std::sync::atomic::Ordering::SeqCst),
        recording_registered,
        repaste_registered,
        diagnostics_state,
    ))
}

#[tauri::command]
fn get_diagnostics_snapshot(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    audio_state: State<'_, AudioState>,
    diagnostics_state: State<'_, diagnostics::DiagnosticsState>,
) -> Result<diagnostics::DiagnosticsSnapshot, String> {
    diagnostics_snapshot_for_app(&app, &settings_state, &audio_state, &diagnostics_state)
}

#[tauri::command]
async fn run_microphone_test(app: AppHandle) -> Result<diagnostics::MicrophoneSignalTest, String> {
    let coordinator: State<DictationCoordinatorState> = app.state();
    let settings_state: State<SettingsState> = app.state();
    let audio_state: State<AudioState> = app.state();
    let diagnostics_state: State<diagnostics::DiagnosticsState> = app.state();
    let _operation_guard = coordinator.lock_operation().await;
    if coordinator.activity().active {
        return Err("Finish the active dictation before testing the microphone.".to_string());
    }
    let selected_device_id = settings_state
        .settings
        .lock()
        .map_err(|error| format!("Failed to acquire settings lock: {error}"))?
        .input_device_id
        .clone();
    let token = diagnostics_state.begin_operation(diagnostics::DiagnosticOperation::MicrophoneTest);
    let result =
        audio::test_input_signal(app.clone(), &audio_state, Some(selected_device_id.as_str()))
            .await;
    match result {
        Ok(levels) => {
            let test = diagnostics::microphone_test_from_levels(&levels);
            diagnostics_state.record_microphone_test(test.clone());
            diagnostics_state.finish_operation(token, Ok(()));
            Ok(test)
        }
        Err(error) => {
            diagnostics_state.finish_operation(token, Err(&error));
            Err(diagnostics::sanitize_error(&error))
        }
    }
}

#[tauri::command]
async fn test_provider_auth(
    app: AppHandle,
) -> Result<diagnostics::ProviderConnectivityTest, String> {
    let settings_state: State<SettingsState> = app.state();
    let http_state: State<HttpClientState> = app.state();
    let diagnostics_state: State<diagnostics::DiagnosticsState> = app.state();
    let settings = settings_state
        .settings
        .lock()
        .map_err(|error| format!("Failed to acquire settings lock: {error}"))?
        .clone();
    let token = diagnostics_state.begin_operation(diagnostics::DiagnosticOperation::ProviderTest);
    let test = diagnostics::test_provider_models(
        &http_state.client,
        &settings.transcription_provider,
        settings.transcription_api_key(),
    )
    .await;
    diagnostics_state.record_provider_test(test.clone());
    let result = if test.authenticated {
        Ok(())
    } else {
        Err(test.error.as_deref().unwrap_or("Provider test failed."))
    };
    diagnostics_state.finish_operation(token, result);
    Ok(test)
}

#[tauri::command]
fn export_diagnostics(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    audio_state: State<'_, AudioState>,
    diagnostics_state: State<'_, diagnostics::DiagnosticsState>,
) -> Result<String, String> {
    let snapshot =
        diagnostics_snapshot_for_app(&app, &settings_state, &audio_state, &diagnostics_state)?;
    let contents = diagnostics::export_diagnostics_json(&snapshot)?;
    let path =
        user_export::write_download(&app, "famvoice-diagnostics", "json", contents.as_bytes())?;
    Ok(path.to_string_lossy().into_owned())
}

fn emit_dictation_activity(app: &AppHandle, coordinator: &DictationCoordinatorState) {
    let _ = app.emit("dictation-activity", coordinator.activity());
}

fn emit_retry_audio_state(app: &AppHandle, state: &retry_audio::RetryAudioState) {
    let _ = app.emit("retry-audio-state", state.status());
}

#[tauri::command]
async fn start_recording_cmd(app: AppHandle) -> Result<(), String> {
    let audio_state: State<AudioState> = app.state();
    let tasks_state: State<BackgroundTasksState> = app.state();
    let settings_state: State<SettingsState> = app.state();
    let coordinator: State<DictationCoordinatorState> = app.state();
    let retry_state: State<retry_audio::RetryAudioState> = app.state();
    let _operation_guard = coordinator.lock_operation().await;
    let input_device_id = settings_state
        .settings
        .lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?
        .input_device_id
        .clone();
    let session_id = coordinator.begin_recording()?;
    retry_state.discard();
    emit_retry_audio_state(&app, &retry_state);
    emit_dictation_activity(&app, &coordinator);
    tasks_state.invalidate_status_reset();

    match audio::start_recording(
        app.clone(),
        &audio_state,
        session_id,
        Some(input_device_id.as_str()),
    )
    .await
    {
        Ok(()) => {
            if let Err(error) = ensure_main_window_visible(&app, false) {
                log_operation_error(
                    "Failed to restore main window after recording start",
                    &error,
                );
            }
            let _ = app.emit("status", "recording");
            let _ = app.emit("transcript", "");
            Ok(())
        }
        Err(error) => {
            coordinator.fail_recording(session_id);
            emit_dictation_activity(&app, &coordinator);
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

fn should_touch_clipboard(_auto_paste: bool, copy_transcript_to_clipboard: bool) -> bool {
    copy_transcript_to_clipboard
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

fn finish_session_with_error(
    app: &AppHandle,
    tasks_state: &BackgroundTasksState,
    coordinator: &DictationCoordinatorState,
    session_id: SessionId,
    message: &str,
) {
    let should_emit = coordinator.finish_session(session_id);
    emit_dictation_activity(app, coordinator);
    if should_emit {
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", message);
        schedule_status_reset(app.clone(), tasks_state);
    }
}

#[tauri::command]
fn get_retry_audio_state(
    state: State<'_, retry_audio::RetryAudioState>,
) -> retry_audio::RetryAudioStatus {
    state.status()
}

#[tauri::command]
fn discard_last_failed_dictation(app: AppHandle, state: State<'_, retry_audio::RetryAudioState>) {
    state.discard();
    emit_retry_audio_state(&app, &state);
}

pub(crate) async fn handle_audio_stream_failure(app: AppHandle, session_id: SessionId) {
    let coordinator: State<DictationCoordinatorState> = app.state();
    let tasks_state: State<BackgroundTasksState> = app.state();
    let _operation_guard = coordinator.lock_operation().await;

    if coordinator.fail_recording(session_id) {
        tasks_state.invalidate_status_reset();
        emit_dictation_activity(&app, &coordinator);
        let message =
            "The microphone stopped unexpectedly. Check the input device and try recording again.";
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", message);
        schedule_status_reset(app.clone(), &tasks_state);
    }
}

struct PreparedRecording {
    settings: AppSettings,
    samples: Vec<i16>,
    silence_threshold: f64,
}

fn prepare_recorded_samples(
    mut samples: Vec<i16>,
    settings_state: &SettingsState,
) -> Result<PreparedRecording, String> {
    if samples.is_empty() {
        eprintln!("[FamVoice] No audio samples recorded");
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

    Ok(PreparedRecording {
        settings,
        samples,
        silence_threshold,
    })
}

struct EncodedRecording {
    bytes: Vec<u8>,
    format: retry_audio::RetryAudioFormat,
    audio_duration: std::time::Duration,
}

impl EncodedRecording {
    fn into_retry_parts(mut self) -> (Vec<u8>, retry_audio::RetryAudioFormat, std::time::Duration) {
        (
            std::mem::take(&mut self.bytes),
            self.format,
            self.audio_duration,
        )
    }
}

impl Drop for EncodedRecording {
    fn drop(&mut self) {
        self.bytes.fill(0);
        std::hint::black_box(self.bytes.as_mut_slice());
    }
}

fn encode_recording(samples: Vec<i16>, silence_threshold: f64) -> EncodedRecording {
    let t_encode = std::time::Instant::now();
    let upload_audio = audio::select_samples_for_upload(&samples, silence_threshold);
    let sample_rate = 16_000.0;
    let audio_duration =
        std::time::Duration::from_secs_f64(upload_audio.samples.len() as f64 / sample_rate);

    if upload_audio.was_trimmed {
        eprintln!(
            "[FamVoice] Speech window trimmed upload from {} samples ({:.1}s) to {} samples ({:.1}s)",
            samples.len(),
            samples.len() as f64 / sample_rate,
            upload_audio.samples.len(),
            upload_audio.samples.len() as f64 / sample_rate
        );
    }

    let (bytes, format, format_label) =
        match audio::encode_flac_in_memory(upload_audio.samples.as_ref()) {
            Ok(flac_bytes) => (flac_bytes, retry_audio::RetryAudioFormat::Flac, "FLAC"),
            Err(flac_err) => {
                eprintln!(
                    "[FamVoice] FLAC encode failed, falling back to WAV: {}",
                    flac_err
                );
                (
                    audio::encode_wav_in_memory(upload_audio.samples.as_ref()),
                    retry_audio::RetryAudioFormat::Wav,
                    "WAV",
                )
            }
        };
    eprintln!(
        "[FamVoice] {} encode: {} samples ({:.1}s) -> {} bytes in {:.0}ms",
        format_label,
        upload_audio.samples.len(),
        upload_audio.samples.len() as f64 / sample_rate,
        bytes.len(),
        t_encode.elapsed().as_secs_f64() * 1000.0
    );

    EncodedRecording {
        bytes,
        format,
        audio_duration,
    }
}

async fn transcribe_encoded_recording(
    http_client: &reqwest::Client,
    settings: &AppSettings,
    audio_bytes: &[u8],
    audio_format: retry_audio::RetryAudioFormat,
    audio_duration: std::time::Duration,
    started_at: std::time::Instant,
) -> Result<String, String> {
    let t_api = std::time::Instant::now();

    if settings.transcription_api_key().trim().is_empty() {
        let provider_label = if settings.transcription_provider == "groq" {
            "Groq"
        } else {
            "OpenAI"
        };
        eprintln!("[FamVoice] {} API key is empty!", provider_label);
        return Err(format!("{} API key missing", provider_label));
    }

    eprintln!(
        "[FamVoice] Transcribing with provider: {}, model: {}, language preference: {}, path: upload",
        settings.transcription_provider, settings.model, settings.language
    );
    let lang = transcription_language_override(&settings.language);
    let transcription_keywords = glossary::transcription_keywords(&settings.replacements);
    let transcription_prompt = if settings.model == "gpt-transcribe" {
        // GPT Transcribe has a dedicated literal keyword field. Keep replacement
        // values out of its unstructured context so hints cannot introduce text
        // that the user did not say.
        glossary::transcription_context_prompt(&settings.language)
    } else {
        // Preserve the established prompt-compatible path for Whisper/Groq.
        glossary::transcription_prompt(&settings.language, &settings.replacements)
    };
    let text = transcription::transcribe_audio(
        http_client,
        audio_bytes.to_vec(),
        settings.transcription_api_key(),
        transcription::TranscriptionRequest {
            model: &settings.model,
            language: lang,
            prompt: transcription_prompt.as_deref(),
            keywords: &transcription_keywords,
            provider: &settings.transcription_provider,
            mime_type: audio_format.mime_type(),
            file_name: audio_format.file_name(),
            audio_duration,
        },
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
    let text = transcription::validate_transcript_text(&text)?;
    eprintln!(
        "[FamVoice] Transcript ready: path=upload | API {:.0}ms | Total {:.0}ms | {} chars",
        t_api.elapsed().as_secs_f64() * 1000.0,
        started_at.elapsed().as_secs_f64() * 1000.0,
        text.chars().count(),
    );

    Ok(text)
}

struct TranscriptDeliveryContext<'a> {
    app: &'a AppHandle,
    tasks_state: &'a BackgroundTasksState,
    coordinator: &'a DictationCoordinatorState,
    history_state: &'a HistoryState,
    clipboard_state: &'a ClipboardState,
}

async fn deliver_transcript(
    context: TranscriptDeliveryContext<'_>,
    session_id: SessionId,
    settings: &AppSettings,
    text: String,
) {
    let TranscriptDeliveryContext {
        app,
        tasks_state,
        coordinator,
        history_state,
        clipboard_state,
    } = context;
    let _operation_guard = coordinator.lock_operation().await;
    let text = match transcription::validate_transcript_text(&text) {
        Ok(text) => text,
        Err(error) => {
            finish_session_with_error(app, tasks_state, coordinator, session_id, &error);
            return;
        }
    };

    if !coordinator.should_deliver(session_id) {
        if let Err(error) = history_state.add(text) {
            log_operation_error(
                "Failed to preserve superseded transcript in history",
                &error,
            );
        }
        emit_history_updated(app, history_state);
        coordinator.finish_session(session_id);
        emit_dictation_activity(app, coordinator);
        return;
    }

    let should_copy_transcript_to_clipboard = settings.preserve_clipboard;
    let should_touch_clipboard =
        should_touch_clipboard(settings.auto_paste, should_copy_transcript_to_clipboard);
    let history_save_error = history_state.add(text.clone()).err();
    emit_history_updated(app, history_state);

    let requires_external_delivery = settings.auto_paste || should_copy_transcript_to_clipboard;
    let length_error = requires_external_delivery
        .then(|| delivery::validate_text_length(&text).err())
        .flatten();
    let delivery_allowed = length_error.is_none();
    let _clipboard_guard = if delivery_allowed && should_touch_clipboard {
        Some(clipboard_state.lock_transaction().await)
    } else {
        None
    };

    let mut delivery_error = length_error;

    if delivery_allowed && should_touch_clipboard {
        if let Err(error) = clipboard::set_clipboard(clipboard_state, &text) {
            eprintln!("[FamVoice] Failed to set clipboard: {}", error);
            delivery_error = Some(format!("Could not copy the transcript: {error}"));
        }
    }

    if settings.auto_paste && delivery_error.is_none() {
        let injection_result = if should_touch_clipboard {
            tokio::time::sleep(paste_clipboard_settle_delay()).await;
            tokio::task::spawn_blocking(injection::simulate_paste).await
        } else {
            let transcript = text.clone();
            tokio::task::spawn_blocking(move || injection::simulate_text(&transcript)).await
        };

        match injection_result {
            Ok(Err(error)) => {
                eprintln!("[FamVoice] Failed to insert transcript: {}", error);
                delivery_error = Some(format!("Could not insert the transcript: {error}"));
            }
            Err(join_error) => {
                let error = format!("Transcript insertion task panicked: {}", join_error);
                eprintln!("[FamVoice] {}", error);
                delivery_error = Some(error);
            }
            Ok(Ok(())) => {}
        }
    }

    if let Some(error) = history_save_error {
        log_operation_error("Failed to save transcript history", &error);
        let persistence_message =
            "The transcript could not be saved to disk. Check available disk space before continuing.";
        delivery_error = Some(match delivery_error {
            Some(existing) => format!("{existing} {persistence_message}"),
            None => persistence_message.to_string(),
        });
    }

    if let Some(error) = delivery_error {
        let _ = app.emit("status", "error");
        let error_msg = if should_copy_transcript_to_clipboard {
            format!(
                "{error}. The transcript is available in History and may still be on the clipboard."
            )
        } else {
            format!("{error}. The transcript is available in History.")
        };
        let _ = app.emit("transcript", error_msg);
    } else {
        let _ = app.emit("transcript", text);
        let _ = app.emit("status", "success");
    }

    coordinator.finish_session(session_id);
    emit_dictation_activity(app, coordinator);
    schedule_status_reset(app.clone(), tasks_state);
}

#[tauri::command]
async fn stop_recording_cmd(app: AppHandle) -> Result<(), String> {
    let audio_state: State<AudioState> = app.state();
    let tasks_state: State<BackgroundTasksState> = app.state();
    let coordinator: State<DictationCoordinatorState> = app.state();
    let settings_state: State<SettingsState> = app.state();
    let history_state: State<HistoryState> = app.state();
    let clipboard_state: State<ClipboardState> = app.state();
    let http_state: State<HttpClientState> = app.state();
    let retry_state: State<retry_audio::RetryAudioState> = app.state();
    let diagnostics_state: State<diagnostics::DiagnosticsState> = app.state();
    let started_at = std::time::Instant::now();

    let (session_id, samples) = {
        let _operation_guard = coordinator.lock_operation().await;
        let session_id = coordinator
            .current_recording_session()
            .ok_or_else(|| "Not recording".to_string())?;
        coordinator.begin_transcription(session_id)?;
        emit_dictation_activity(&app, &coordinator);
        tasks_state.invalidate_status_reset();
        let _ = app.emit("status", "transcribing");

        let Some(samples) = audio::stop_recording(&audio_state, session_id).await else {
            let message = if audio_state
                .stream_healthy
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                "Recording could not be completed"
            } else {
                "The microphone stopped unexpectedly. Check the input device and try recording again."
            };
            finish_session_with_error(&app, &tasks_state, &coordinator, session_id, message);
            return Err(message.to_string());
        };
        (session_id, samples)
    };
    let diagnostics_token =
        diagnostics_state.begin_operation(diagnostics::DiagnosticOperation::Dictation);

    let prepared = match prepare_recorded_samples(samples, &settings_state) {
        Ok(prepared) => prepared,
        Err(error) => {
            diagnostics_state.finish_operation(diagnostics_token, Err(&error));
            let _operation_guard = coordinator.lock_operation().await;
            finish_session_with_error(&app, &tasks_state, &coordinator, session_id, &error);
            return Err(error);
        }
    };
    let encoded = encode_recording(prepared.samples, prepared.silence_threshold);
    let text = match transcribe_encoded_recording(
        &http_state.client,
        &prepared.settings,
        &encoded.bytes,
        encoded.format,
        encoded.audio_duration,
        started_at,
    )
    .await
    {
        Ok(text) => text,
        Err(error) => {
            eprintln!("[FamVoice] Transcription error: {}", error);
            diagnostics_state.finish_operation(diagnostics_token, Err(&error));
            let _operation_guard = coordinator.lock_operation().await;
            if coordinator.should_deliver(session_id) {
                let (bytes, format, audio_duration) = encoded.into_retry_parts();
                if let Err(cache_error) = retry_state.store(bytes, format, audio_duration) {
                    log_operation_error("Failed to retain temporary retry audio", &cache_error);
                }
                emit_retry_audio_state(&app, &retry_state);
            }
            finish_session_with_error(&app, &tasks_state, &coordinator, session_id, &error);
            return Err(error);
        }
    };

    deliver_transcript(
        TranscriptDeliveryContext {
            app: &app,
            tasks_state: &tasks_state,
            coordinator: &coordinator,
            history_state: &history_state,
            clipboard_state: &clipboard_state,
        },
        session_id,
        &prepared.settings,
        text,
    )
    .await;
    diagnostics_state.finish_operation(diagnostics_token, Ok(()));
    Ok(())
}

#[tauri::command]
async fn retry_last_dictation(app: AppHandle) -> Result<(), String> {
    let tasks_state: State<BackgroundTasksState> = app.state();
    let coordinator: State<DictationCoordinatorState> = app.state();
    let settings_state: State<SettingsState> = app.state();
    let history_state: State<HistoryState> = app.state();
    let clipboard_state: State<ClipboardState> = app.state();
    let http_state: State<HttpClientState> = app.state();
    let retry_state: State<retry_audio::RetryAudioState> = app.state();
    let diagnostics_state: State<diagnostics::DiagnosticsState> = app.state();
    let started_at = std::time::Instant::now();

    let (session_id, audio, settings) = {
        let _operation_guard = coordinator.lock_operation().await;
        let settings = settings_state
            .settings
            .lock()
            .map_err(|error| format!("Failed to acquire settings lock: {error}"))?
            .clone();
        let session_id = coordinator.begin_retry_transcription()?;
        let Some(audio) = retry_state.take() else {
            coordinator.finish_session(session_id);
            emit_dictation_activity(&app, &coordinator);
            emit_retry_audio_state(&app, &retry_state);
            return Err("The failed dictation is no longer available".to_string());
        };
        tasks_state.invalidate_status_reset();
        emit_retry_audio_state(&app, &retry_state);
        emit_dictation_activity(&app, &coordinator);
        let _ = app.emit("status", "transcribing");
        let _ = app.emit("transcript", "");
        (session_id, audio, settings)
    };

    let diagnostics_token =
        diagnostics_state.begin_operation(diagnostics::DiagnosticOperation::Dictation);
    let result = transcribe_encoded_recording(
        &http_state.client,
        &settings,
        audio.bytes(),
        audio.format(),
        audio.audio_duration(),
        started_at,
    )
    .await;

    let text = match result {
        Ok(text) => text,
        Err(error) => {
            diagnostics_state.finish_operation(diagnostics_token, Err(&error));
            let _operation_guard = coordinator.lock_operation().await;
            finish_session_with_error(&app, &tasks_state, &coordinator, session_id, &error);
            return Err(error);
        }
    };

    deliver_transcript(
        TranscriptDeliveryContext {
            app: &app,
            tasks_state: &tasks_state,
            coordinator: &coordinator,
            history_state: &history_state,
            clipboard_state: &clipboard_state,
        },
        session_id,
        &settings,
        text,
    )
    .await;
    diagnostics_state.finish_operation(diagnostics_token, Ok(()));
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
    fn copy_disabled_auto_paste_does_not_touch_clipboard() {
        assert!(
            !should_touch_clipboard(true, false),
            "auto-paste must not write dictated text to the clipboard when copy is disabled",
        );
        assert!(!should_touch_clipboard(false, false));
        assert!(should_touch_clipboard(true, true));
        assert!(should_touch_clipboard(false, true));
    }

    #[test]
    fn microphone_change_is_rejected_during_recording_and_transcription() {
        for activity in [
            DictationActivity {
                active: true,
                recording: true,
                transcribing: false,
            },
            DictationActivity {
                active: true,
                recording: false,
                transcribing: true,
            },
        ] {
            let error = ensure_input_device_change_allowed("mic-a", "mic-b", activity)
                .expect_err("active dictation must reject microphone changes");
            assert!(error.contains("recording or transcription is active"));
        }
    }

    #[test]
    fn microphone_change_is_allowed_when_idle_or_unchanged() {
        let idle = DictationActivity {
            active: false,
            recording: false,
            transcribing: false,
        };
        let recording = DictationActivity {
            active: true,
            recording: true,
            transcribing: false,
        };

        assert!(ensure_input_device_change_allowed("mic-a", "mic-b", idle).is_ok());
        assert!(ensure_input_device_change_allowed("mic-a", "mic-a", recording).is_ok());
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
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            startup::disable_unsafe_autostart_entry(app.handle());

            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&app_dir).unwrap_or_default();

            app.manage(DictationCoordinatorState::default());
            app.manage(AudioState::default());
            app.manage(diagnostics::DiagnosticsState::default());
            app.manage(retry_audio::RetryAudioState::default());
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
                        if let Err(error) = ensure_main_window_visible(app, true) {
                            log_operation_error("Failed to show main window from tray", &error);
                        }
                    }
                    TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        let app = tray.app_handle();
                        if let Err(error) = ensure_main_window_visible(app, true) {
                            log_operation_error("Failed to show main window from tray", &error);
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
            get_dictation_activity,
            save_settings,
            list_input_devices,
            get_history,
            delete_history_item,
            restore_history_item,
            clear_history,
            get_history_retention,
            set_history_retention,
            toggle_history_pin,
            export_history,
            repaste_history_item,
            get_retry_audio_state,
            retry_last_dictation,
            discard_last_failed_dictation,
            get_diagnostics_snapshot,
            run_microphone_test,
            test_provider_auth,
            export_diagnostics,
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
