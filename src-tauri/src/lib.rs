mod audio;
mod clipboard;
mod history;
mod injection;
mod input_hook;
mod prompt_optimizer;
mod settings;
mod transcription;

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use audio::AudioState;
use clipboard::ClipboardState;
use history::HistoryState;
use settings::{validate_settings, AppSettings, SettingsState};

const PASTE_CLIPBOARD_SETTLE_DELAY_MS: u64 = 5;
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 40;
const PROMPT_OPTIMIZER_TIMEOUT_MS: u64 = 10_000;
const PROMPT_OPTIMIZER_SLOW_MODEL_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_WINDOW_WIDTH: f64 = 260.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 200.0;
const DEFAULT_WIDGET_WIDTH: f64 = 128.0;
const DEFAULT_WIDGET_HEIGHT: f64 = 44.0;

pub struct HttpClientState {
    pub client: reqwest::Client,
}

pub struct BackgroundTasksState {
    handles: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl BackgroundTasksState {
    fn new() -> Self {
        Self {
            handles: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn spawn(&self, handle: tokio::task::JoinHandle<()>) {
        if let Ok(mut handles) = self.handles.lock() {
            handles.push(handle);
            if handles.len() > 10 {
                handles.remove(0);
            }
        }
    }
}

use input_hook::HotkeyConfigState;
use std::sync::Mutex;

fn register_hotkey(app: &AppHandle, hotkey: &str) {
    input_hook::reset_mouse_hotkey_state();

    // Update global mouse listener config
    if let Some(state) = app.try_state::<HotkeyConfigState>() {
        *state.hotkey.lock().unwrap() = hotkey.to_string();
    }

    // Unregister all existing keyboard shortcuts
    let _ = app.global_shortcut().unregister_all();

    if input_hook::is_mouse_hotkey(hotkey) {
        eprintln!("[FamVoice] Mouse hotkey registered globally: {}", hotkey);
        return;
    }

    if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
        let _ = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let state: State<AudioState> = app.state();
                    let is_recording = state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                    if !is_recording {
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let audio_state: State<AudioState> = app_clone.state();
                            let _ = start_recording_cmd(app_clone.clone(), audio_state).await;
                        });
                    }
                } else if event.state() == ShortcutState::Released {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let audio_state: State<AudioState> = app_clone.state();
                        let settings_state: State<SettingsState> = app_clone.state();
                        let history_state: State<HistoryState> = app_clone.state();
                        let clipboard_state: State<ClipboardState> = app_clone.state();
                        let http_state: State<HttpClientState> = app_clone.state();
                        let tasks_state: State<BackgroundTasksState> = app_clone.state();
                        let _ = stop_recording_cmd(
                            app_clone.clone(),
                            tasks_state,
                            audio_state,
                            settings_state,
                            history_state,
                            clipboard_state,
                            http_state,
                        )
                        .await;
                    });
                }
            });
    } else {
        eprintln!("[FamVoice] Failed to parse hotkey: {}", hotkey);
    }
}

#[tauri::command]
fn get_settings(state: State<'_, SettingsState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

fn main_window_dimensions(widget_mode: bool) -> (f64, f64) {
    if widget_mode {
        (DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT)
    } else {
        (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
    }
}

fn set_main_window_size(
    window: &WebviewWindow,
    width: f64,
    height: f64,
    center: bool,
) -> Result<(), String> {
    window.set_resizable(true).map_err(|e| e.to_string())?;
    window
        .set_min_size(None::<Size>)
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(None::<Size>)
        .map_err(|e| e.to_string())?;

    let size = LogicalSize::new(width, height);
    window.set_size(size).map_err(|e| e.to_string())?;
    window
        .set_min_size(Some(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(Some(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window.set_resizable(false).map_err(|e| e.to_string())?;
    let _ = window.set_maximizable(false);

    if center {
        let _ = window.center();
    }

    Ok(())
}

fn apply_main_window_mode(app: &AppHandle, widget_mode: bool, center: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    let (width, height) = main_window_dimensions(widget_mode);
    set_main_window_size(&window, width, height, center)
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
    new_settings: AppSettings,
) -> Result<(), String> {
    if let Err(errors) = validate_settings(&new_settings) {
        return Err(format!("Invalid settings: {}", errors.join(", ")));
    }

    let (old_settings, old_hotkey) = {
        let mut settings = state.settings.lock().unwrap();
        let old_settings = settings.clone();
        let old_hotkey = old_settings.hotkey.clone();
        *settings = new_settings.clone();
        (old_settings, old_hotkey)
    };
    if let Err(e) = state.save() {
        *state.settings.lock().unwrap() = old_settings;
        return Err(format!("Failed to save settings: {}", e));
    }

    let _ = app.emit("settings-updated", new_settings.clone());

    if old_settings.widget_mode != new_settings.widget_mode {
        apply_main_window_mode(&app, new_settings.widget_mode, true)?;
    }

    if old_hotkey != new_settings.hotkey {
        register_hotkey(&app, &new_settings.hotkey);
    }
    Ok(())
}

#[tauri::command]
fn resize_main_window(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    set_main_window_size(&window, width, height, false)
}

#[tauri::command]
fn get_history(state: State<'_, HistoryState>) -> Vec<history::HistoryItem> {
    state.items.lock().unwrap().clone()
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
async fn repaste_history_item(
    _clipboard_state: State<'_, ClipboardState>,
    text: String,
) -> Result<(), String> {
    if let Err(e) = clipboard::set_clipboard(&text) {
        return Err(format!("Failed to set clipboard: {}", e));
    }
    tokio::time::sleep(paste_clipboard_settle_delay()).await;
    if let Err(e) = injection::simulate_paste() {
        return Err(format!("Failed to simulate paste: {}", e));
    }
    Ok(())
}

#[tauri::command]
async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("Settings")
    .inner_size(340.0, 520.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn transcription_language_override(language_preference: &str) -> Option<&str> {
    match language_preference {
        "pt" => Some("pt"),
        "en" => Some("en"),
        _ => None,
    }
}

#[tauri::command]
async fn start_recording_cmd(
    app: AppHandle,
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    match audio::start_recording(&*audio_state).await {
        Ok(()) => {
            let _ = app.emit("status", "recording");
            Ok(())
        }
        Err(e) => {
            eprintln!("[FamVoice] Failed to start recording: {}", e);
            let _ = app.emit("status", "error");
            let _ = app.emit("transcript", e.clone());
            Err(e)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GlossaryRule {
    target: String,
    replacement: String,
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '_'
}

fn is_single_word_target(target: &str) -> bool {
    !target.is_empty() && target.chars().all(is_word_char)
}

fn sorted_glossary_rules(
    replacements: &[settings::Replacement],
) -> (Vec<GlossaryRule>, Vec<GlossaryRule>) {
    let mut phrase_rules = Vec::new();
    let mut single_word_rules = Vec::new();

    for replacement in replacements {
        let target = replacement.target.trim();
        if target.is_empty() {
            continue;
        }

        let rule = GlossaryRule {
            target: target.to_string(),
            replacement: replacement.replacement.clone(),
        };

        if is_single_word_target(target) {
            single_word_rules.push(rule);
        } else {
            phrase_rules.push(rule);
        }
    }

    let sort_rules = |rules: &mut Vec<GlossaryRule>| {
        rules.sort_by(|left, right| {
            right
                .target
                .chars()
                .count()
                .cmp(&left.target.chars().count())
                .then_with(|| left.target.cmp(&right.target))
        });
    };

    sort_rules(&mut phrase_rules);
    sort_rules(&mut single_word_rules);
    (phrase_rules, single_word_rules)
}

fn replace_whole_word_case_insensitive(text: &str, target: &str, replacement: &str) -> String {
    let target_lower = target.to_lowercase();
    let mut output = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.peek().copied() {
        if !is_word_char(ch) {
            output.push(ch);
            chars.next();
            continue;
        }

        let mut token = String::new();
        while let Some((_, word_char)) = chars.peek().copied() {
            if !is_word_char(word_char) {
                break;
            }
            token.push(word_char);
            chars.next();
        }

        if token.to_lowercase() == target_lower {
            output.push_str(replacement);
        } else {
            output.push_str(&token);
        }
    }

    output
}

fn finalize_transcript(mut text: String, replacements: &[settings::Replacement]) -> String {
    let (phrase_rules, single_word_rules) = sorted_glossary_rules(replacements);

    for rule in phrase_rules {
        text = text.replace(&rule.target, &rule.replacement);
    }

    for rule in single_word_rules {
        text = replace_whole_word_case_insensitive(&text, &rule.target, &rule.replacement);
    }

    if text.ends_with('.') {
        text.pop();
    }

    text
}

fn prompt_optimizer_timeout(model: &str) -> std::time::Duration {
    let timeout_ms = match model {
        "claude-sonnet-4-6" => PROMPT_OPTIMIZER_SLOW_MODEL_TIMEOUT_MS,
        _ => PROMPT_OPTIMIZER_TIMEOUT_MS,
    };

    std::time::Duration::from_millis(timeout_ms)
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

    let api_key = settings.anthropic_api_key.trim();
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

const MIC_MIN_SILENCE_THRESHOLD_RMS: f64 = 42.0;
const MIC_MAX_SILENCE_THRESHOLD_RMS: f64 = 12.0;
const MIC_MIN_TARGET_RMS: f64 = 1200.0;
const MIC_MAX_TARGET_RMS: f64 = 2200.0;
const MIC_TARGET_PEAK: f64 = 12000.0;
const MIC_MAX_AUTO_GAIN: f64 = 8.0;
const MIC_MIN_AUTO_GAIN_TO_APPLY: f64 = 1.2;

#[derive(Clone, Copy, Debug)]
struct MicAudioLevels {
    rms: f64,
    peak: f64,
}

#[derive(Clone, Copy, Debug)]
struct MicLevelDetails {
    rms_dbfs: f64,
    peak_percent: f64,
}

fn mic_interpolate(min_value: f64, max_value: f64, mic_sensitivity: u8) -> f64 {
    let ratio = f64::from(mic_sensitivity.min(settings::MAX_MIC_SENSITIVITY))
        / f64::from(settings::MAX_MIC_SENSITIVITY);
    min_value + (max_value - min_value) * ratio
}

fn mic_silence_threshold(mic_sensitivity: u8) -> f64 {
    mic_interpolate(
        MIC_MIN_SILENCE_THRESHOLD_RMS,
        MIC_MAX_SILENCE_THRESHOLD_RMS,
        mic_sensitivity,
    )
}

fn mic_target_rms(mic_sensitivity: u8) -> f64 {
    mic_interpolate(MIC_MIN_TARGET_RMS, MIC_MAX_TARGET_RMS, mic_sensitivity)
}

fn mic_analyze_levels(samples: &[i16]) -> MicAudioLevels {
    if samples.is_empty() {
        return MicAudioLevels {
            rms: 0.0,
            peak: 0.0,
        };
    }

    let mut sum_squares = 0.0;
    let mut peak: f64 = 0.0;

    for &sample in samples {
        let sample_f64 = sample as f64;
        sum_squares += sample_f64 * sample_f64;
        peak = peak.max((sample as i32).abs() as f64);
    }

    MicAudioLevels {
        rms: (sum_squares / samples.len() as f64).sqrt(),
        peak,
    }
}

fn mic_dbfs(level: f64) -> f64 {
    if level <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * (level / i16::MAX as f64).log10()
    }
}

fn mic_level_details(levels: MicAudioLevels) -> MicLevelDetails {
    MicLevelDetails {
        rms_dbfs: mic_dbfs(levels.rms),
        peak_percent: (levels.peak / i16::MAX as f64 * 100.0).clamp(0.0, 100.0),
    }
}

fn mic_should_reject_for_silence(levels: MicAudioLevels, mic_sensitivity: u8) -> bool {
    levels.rms < mic_silence_threshold(mic_sensitivity)
}

fn mic_auto_gain(levels: MicAudioLevels, mic_sensitivity: u8) -> Option<f64> {
    if levels.rms <= 0.0 || levels.peak <= 0.0 {
        return None;
    }

    let target_rms = mic_target_rms(mic_sensitivity);
    if levels.rms >= target_rms {
        return None;
    }

    let gain = (target_rms / levels.rms)
        .min(MIC_TARGET_PEAK / levels.peak)
        .min(MIC_MAX_AUTO_GAIN);

    (gain > MIC_MIN_AUTO_GAIN_TO_APPLY).then_some(gain)
}

fn mic_apply_gain(samples: &mut [i16], gain: f64) {
    for sample in samples {
        *sample = ((*sample as f64) * gain)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
    }
}

fn mic_normalize_quiet_audio(samples: &mut [i16], mic_sensitivity: u8) -> Option<f64> {
    let gain = mic_auto_gain(mic_analyze_levels(samples), mic_sensitivity)?;
    mic_apply_gain(samples, gain);
    Some(gain)
}

fn should_restore_clipboard(
    auto_paste: bool,
    preserve_clipboard: bool,
    paste_successful: bool,
) -> bool {
    auto_paste && preserve_clipboard && paste_successful
}

fn should_store_history(auto_paste: bool, paste_successful: bool) -> bool {
    auto_paste && paste_successful
}

fn paste_clipboard_settle_delay() -> std::time::Duration {
    std::time::Duration::from_millis(PASTE_CLIPBOARD_SETTLE_DELAY_MS)
}

fn clipboard_restore_delay() -> std::time::Duration {
    std::time::Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TranscriptPath {
    Upload,
}

fn transcript_path_label(path: TranscriptPath) -> &'static str {
    match path {
        TranscriptPath::Upload => "upload",
    }
}

fn upload_transcript_path(_model: &str) -> TranscriptPath {
    TranscriptPath::Upload
}

fn emit_history_updated(app: &AppHandle, history_state: &HistoryState) {
    if let Ok(history) = history_state.items.lock().map(|items| items.clone()) {
        let _ = app.emit("history-updated", history);
    }
}

#[tauri::command]
async fn stop_recording_cmd(
    app: AppHandle,
    tasks_state: State<'_, BackgroundTasksState>,
    audio_state: State<'_, AudioState>,
    settings_state: State<'_, SettingsState>,
    history_state: State<'_, HistoryState>,
    clipboard_state: State<'_, ClipboardState>,
    http_state: State<'_, HttpClientState>,
) -> Result<(), String> {
    let t_total = std::time::Instant::now();
    let _ = app.emit("status", "transcribing");
    let mut samples = match audio::stop_recording(&*audio_state).await {
        Some(s) => s,
        None => {
            eprintln!("[FamVoice] stop_recording returned None — was not recording");
            let _ = app.emit("status", "idle");
            return Err("Not recording".into());
        }
    };

    if samples.is_empty() {
        eprintln!("[FamVoice] No audio samples recorded");
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", "No audio recorded");
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            let _ = app_clone.emit("status", "idle");
        });
        tasks_state.spawn(handle);
        return Err("No audio recorded".into());
    }

    let settings = settings_state.settings.lock().unwrap().clone();
    let levels = mic_analyze_levels(&samples);
    let silence_threshold = mic_silence_threshold(settings.mic_sensitivity);
    let level_details = mic_level_details(levels);
    let silence_threshold_dbfs = mic_dbfs(silence_threshold);
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

    if mic_should_reject_for_silence(levels, settings.mic_sensitivity) {
        eprintln!("[FamVoice] Silence detected, skipping transcription");
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", "No voice detected");

        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            let _ = app_clone.emit("status", "idle");
        });
        tasks_state.spawn(handle);
        return Err("No voice detected".into());
    }

    if let Some(gain) = mic_normalize_quiet_audio(&mut samples, settings.mic_sensitivity) {
        let boosted_levels = mic_analyze_levels(&samples);
        let boosted_details = mic_level_details(boosted_levels);
        eprintln!(
            "[FamVoice] Applied mic gain {:.2}x -> rms {:.2} ({:.1} dBFS), peak {:.0} ({:.1}%)",
            gain,
            boosted_levels.rms,
            boosted_details.rms_dbfs,
            boosted_levels.peak,
            boosted_details.peak_percent
        );
    }

    if settings.api_key.is_empty() {
        eprintln!("[FamVoice] API key is empty!");
        let _ = app.emit("status", "error");
        let _ = app.emit("transcript", "API key is empty. Set it in Settings.");
        return Err("API key is empty".into());
    }

    let t_encode = std::time::Instant::now();
    let upload_audio = audio::select_samples_for_upload(&samples, silence_threshold);
    if upload_audio.was_trimmed {
        eprintln!(
            "[FamVoice] Speech window trimmed upload from {} samples ({:.1}s) to {} samples ({:.1}s)",
            samples.len(),
            samples.len() as f64 / 16000.0,
            upload_audio.samples.len(),
            upload_audio.samples.len() as f64 / 16000.0
        );
    }

    let wav_bytes = audio::encode_wav_in_memory(&upload_audio.samples);
    let t_api = std::time::Instant::now();
    eprintln!(
        "[FamVoice] WAV encode: {} samples ({:.1}s) -> {} bytes in {:.0}ms",
        upload_audio.samples.len(),
        upload_audio.samples.len() as f64 / 16000.0,
        wav_bytes.len(),
        t_encode.elapsed().as_secs_f64() * 1000.0
    );

    let transcript_path = upload_transcript_path(&settings.model);
    eprintln!(
        "[FamVoice] Transcribing with model: {}, language preference: {}, path: {}",
        settings.model,
        settings.language,
        transcript_path_label(transcript_path)
    );
    let lang = transcription_language_override(&settings.language);
    let transcription_result = transcription::transcribe_audio(
        &http_state.client,
        wav_bytes,
        &settings.api_key,
        &settings.model,
        lang,
    )
    .await
    .map(|text| (transcript_path, text));
    drop(samples);

    match transcription_result {
        Ok((transcript_path, text)) => {
            let finalized_text = finalize_transcript(text, &settings.replacements);
            let text = resolve_final_output_for_paste(
                &settings,
                finalized_text,
                prompt_optimizer_timeout(&settings.prompt_optimizer_model),
                |request| {
                    prompt_optimizer::optimize_prompt(
                        &http_state.client,
                        settings.anthropic_api_key.trim(),
                        request,
                    )
                },
            )
            .await;
            let preview = if text.len() > 100 {
                &text[..100]
            } else {
                &text
            };
            eprintln!("[FamVoice] Transcript ready: path={} | API {:.0}ms | Total {:.0}ms | Result ({} chars): {:?}",
                transcript_path_label(transcript_path),
                t_api.elapsed().as_secs_f64() * 1000.0,
                t_total.elapsed().as_secs_f64() * 1000.0,
                text.len(), preview);

            if settings.auto_paste && settings.preserve_clipboard {
                clipboard::save_clipboard(&*clipboard_state);
            }

            if let Err(e) = clipboard::set_clipboard(&text) {
                eprintln!("[FamVoice] Failed to set clipboard: {}", e);
            }

            let mut paste_successful = true;
            let mut paste_error = None;

            if settings.auto_paste {
                tokio::time::sleep(paste_clipboard_settle_delay()).await;
                if let Err(e) = injection::simulate_paste() {
                    eprintln!("[FamVoice] Failed to simulate paste: {}", e);
                    paste_successful = false;
                    paste_error = Some(e);
                }
            }

            if should_restore_clipboard(
                settings.auto_paste,
                settings.preserve_clipboard,
                paste_successful,
            ) {
                let saved_clipboard = clipboard::saved_clipboard_text(&*clipboard_state);
                let handle = tokio::spawn(async move {
                    tokio::time::sleep(clipboard_restore_delay()).await;
                    if let Some(text) = saved_clipboard {
                        if let Err(error) = clipboard::restore_clipboard_text(&text) {
                            eprintln!("[FamVoice] Failed to restore clipboard: {}", error);
                        }
                    }
                });
                tasks_state.spawn(handle);
            }

            if should_store_history(settings.auto_paste, paste_successful) {
                history_state.add(text.clone());
                emit_history_updated(&app, &history_state);
            }

            if !paste_successful {
                let _ = app.emit("status", "error");
                let error_msg = format!(
                    "Paste failed: {}. Transcript is on clipboard.",
                    paste_error.unwrap_or_default()
                );
                let _ = app.emit("transcript", error_msg);
            } else {
                let _ = app.emit("transcript", text);
                let _ = app.emit("status", "success");
            }
        }
        Err(e) => {
            eprintln!("[FamVoice] Transcription error: {}", e);
            let _ = app.emit("status", "error");
            let _ = app.emit("transcript", e.clone());
            return Err(e);
        }
    }

    let app_clone = app.clone();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let _ = app_clone.emit("status", "idle");
    });
    tasks_state.spawn(handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;
    use crate::settings::Replacement;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn test_upload_transcript_path_treats_gpt_4o_mini_transcribe_as_upload() {
        assert_eq!(
            upload_transcript_path("gpt-4o-mini-transcribe"),
            TranscriptPath::Upload
        );
    }

    #[test]
    fn test_finalize_transcript_applies_replacements_and_trims_trailing_period() {
        let transcript = finalize_transcript(
            "omg hello.".to_string(),
            &[Replacement {
                target: "omg".to_string(),
                replacement: "Oh my gosh".to_string(),
            }],
        );

        assert_eq!(transcript, "Oh my gosh hello");
    }

    #[test]
    fn test_finalize_transcript_skips_blank_replacement_targets() {
        let transcript = finalize_transcript(
            "hello".to_string(),
            &[Replacement {
                target: "   ".to_string(),
                replacement: "ignored".to_string(),
            }],
        );

        assert_eq!(transcript, "hello");
    }

    #[test]
    fn test_finalize_transcript_replaces_single_words_without_touching_substrings() {
        let transcript = finalize_transcript(
            "partial art party".to_string(),
            &[Replacement {
                target: "art".to_string(),
                replacement: "design".to_string(),
            }],
        );

        assert_eq!(transcript, "partial design party");
    }

    #[test]
    fn test_finalize_transcript_replaces_single_words_case_insensitively() {
        let transcript = finalize_transcript(
            "OMG hello".to_string(),
            &[Replacement {
                target: "omg".to_string(),
                replacement: "Oh my gosh".to_string(),
            }],
        );

        assert_eq!(transcript, "Oh my gosh hello");
    }

    #[test]
    fn test_finalize_transcript_prefers_longer_phrase_rules_before_single_words() {
        let transcript = finalize_transcript(
            "new york is new".to_string(),
            &[
                Replacement {
                    target: "new".to_string(),
                    replacement: "fresh".to_string(),
                },
                Replacement {
                    target: "new york".to_string(),
                    replacement: "NYC".to_string(),
                },
            ],
        );

        assert_eq!(transcript, "NYC is fresh");
    }

    #[test]
    fn test_transcription_language_override_keeps_preference_modes_unset() {
        assert_eq!(transcription_language_override("auto"), None);
        assert_eq!(transcription_language_override("pt"), Some("pt"));
        assert_eq!(transcription_language_override("en"), Some("en"));
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
            std::time::Duration::from_millis(5)
        );
    }

    #[test]
    fn test_clipboard_restore_happens_after_short_background_delay() {
        assert_eq!(
            clipboard_restore_delay(),
            std::time::Duration::from_millis(40)
        );
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
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: "sk-anthropic-test".to_string(),
            ..AppSettings::default()
        };

        let output = resolve_final_output_for_paste(
            &settings,
            "final transcript".to_string(),
            std::time::Duration::from_millis(50),
            |request| async move {
                assert_eq!(request.model, "claude-haiku-4-5");
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
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: "sk-anthropic-test".to_string(),
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
    async fn test_resolve_final_output_skips_optimizer_when_anthropic_key_is_blank() {
        let settings = AppSettings {
            prompt_optimization_enabled: true,
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: "   ".to_string(),
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
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: "sk-anthropic-test".to_string(),
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
    fn test_prompt_optimizer_timeout_keeps_haiku_fast() {
        assert_eq!(
            prompt_optimizer_timeout("claude-haiku-4-5").as_millis(),
            10_000
        );
    }

    #[test]
    fn test_prompt_optimizer_timeout_gives_sonnet_more_time() {
        assert_eq!(
            prompt_optimizer_timeout("claude-sonnet-4-6").as_millis(),
            30_000
        );
    }

    #[test]
    fn test_prompt_optimizer_timeout_message_includes_model_name() {
        let message = prompt_optimizer_timeout_message(
            "claude-sonnet-4-6",
            std::time::Duration::from_millis(30_000),
        );

        assert!(message.contains("claude-sonnet-4-6"));
        assert!(message.contains("30000ms"));
        assert!(message.contains("using finalized transcript"));
    }

    #[test]
    fn test_prompt_optimizer_start_message_includes_model_name() {
        let message = prompt_optimizer_start_message("claude-haiku-4-5");

        assert!(message.contains("claude-haiku-4-5"));
        assert!(message.contains("Starting prompt optimization"));
    }

    #[test]
    fn test_prompt_optimizer_success_message_includes_model_name_and_duration() {
        let message = prompt_optimizer_success_message(
            "claude-sonnet-4-6",
            std::time::Duration::from_millis(1842),
        );

        assert!(message.contains("claude-sonnet-4-6"));
        assert!(message.contains("1842ms"));
        assert!(message.contains("succeeded"));
    }

    #[test]
    fn test_prompt_optimizer_failure_message_includes_model_name_and_error() {
        let message = prompt_optimizer_failure_message("claude-haiku-4-5", "request failed");

        assert!(message.contains("claude-haiku-4-5"));
        assert!(message.contains("request failed"));
        assert!(message.contains("using finalized transcript"));
    }

    #[test]
    fn test_main_window_dimensions_use_compact_widget_size() {
        assert_eq!(
            main_window_dimensions(true),
            (DEFAULT_WIDGET_WIDTH, DEFAULT_WIDGET_HEIGHT)
        );
        assert_eq!(
            main_window_dimensions(false),
            (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
        );
    }

    #[test]
    fn mic_sensitivity_lowers_the_silence_threshold() {
        assert!(mic_silence_threshold(100) < mic_silence_threshold(0));
    }

    #[test]
    fn mic_rejects_near_silent_audio_at_default_sensitivity() {
        let levels = mic_analyze_levels(&[0, 10, -10, 0, 5, -5]);

        assert!(mic_should_reject_for_silence(
            levels,
            settings::DEFAULT_MIC_SENSITIVITY,
        ));
    }

    #[test]
    fn mic_high_sensitivity_keeps_quiet_speech_that_low_sensitivity_rejects() {
        let levels = mic_analyze_levels(&[20, -20, 15, -15, 25, -25]);

        assert!(mic_should_reject_for_silence(levels, 0));
        assert!(!mic_should_reject_for_silence(levels, 100));
    }

    #[test]
    fn mic_normalize_quiet_audio_boosts_samples_with_a_gain_cap() {
        let mut samples = vec![100, -100, 50, -50];

        let gain = mic_normalize_quiet_audio(&mut samples, 100).unwrap();

        assert_eq!(gain, 8.0);
        assert_eq!(samples, vec![800, -800, 400, -400]);
    }

    #[test]
    fn mic_level_details_include_dbfs_and_peak_percent() {
        let details = mic_level_details(MicAudioLevels {
            rms: 1024.0,
            peak: 8192.0,
        });

        assert!(details.rms_dbfs < 0.0);
        assert!(details.rms_dbfs > -40.0);
        assert!((details.peak_percent - 25.0).abs() < 0.1);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
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

            // Pre-warm HTTPS connection to OpenAI (TCP + TLS handshake in background)
            // Saves ~100-300ms on the first transcription request
            let warmup_client = http_client.clone();
            tauri::async_runtime::spawn(async move {
                let _ = warmup_client
                    .head("https://api.openai.com/v1/models")
                    .send()
                    .await;
                eprintln!("[FamVoice] HTTPS connection to OpenAI pre-warmed");
            });

            app.manage(HttpClientState {
                client: http_client,
            });

            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .tooltip("FamVoice")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
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

            // Register initial shortcut and start mouse listener
            let hotkey = {
                let state: State<SettingsState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings.hotkey.clone()
            };

            let widget_mode = {
                let state: State<SettingsState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings.widget_mode
            };

            register_hotkey(app.handle(), &hotkey);
            input_hook::start_mouse_listener(app.handle().clone(), hotkey_shared);
            apply_main_window_mode(app.handle(), widget_mode, false)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_history,
            delete_history_item,
            clear_history,
            repaste_history_item,
            start_recording_cmd,
            stop_recording_cmd,
            resize_main_window,
            open_settings_window,
            close_settings_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
