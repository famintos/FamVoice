mod audio;
mod transcription;
mod clipboard;
mod injection;
mod settings;
mod history;
mod input_hook;


use tauri::{Manager, AppHandle, State, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri::tray::{TrayIconBuilder, MouseButton, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use std::path::PathBuf;

use audio::AudioState;
use settings::{SettingsState, AppSettings};
use history::HistoryState;
use clipboard::ClipboardState;

/// Shared HTTP client for connection reuse (avoids TLS handshake per request)
pub struct HttpClientState {
    pub client: reqwest::Client,
}

use input_hook::HotkeyConfigState;
use std::sync::{Arc, Mutex};



fn register_hotkey(app: &AppHandle, hotkey: &str) {
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
        let _ = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {

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
                    let _ = stop_recording_cmd(app_clone.clone(), audio_state, settings_state, history_state, clipboard_state, http_state).await;
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

#[tauri::command]
async fn save_settings(app: AppHandle, state: State<'_, SettingsState>, new_settings: AppSettings) -> Result<(), String> {
    let old_hotkey = state.settings.lock().unwrap().hotkey.clone();
    *state.settings.lock().unwrap() = new_settings.clone();
    state.save();

    // Notify frontend about settings update
    let _ = app.emit("settings-updated", new_settings.clone());

    // Re-register global shortcut if hotkey changed
    if old_hotkey != new_settings.hotkey {
        register_hotkey(&app, &new_settings.hotkey);
    }
    Ok(())
}

#[tauri::command]
fn get_history(state: State<'_, HistoryState>) -> Vec<history::HistoryItem> {
    state.items.lock().unwrap().clone()
}

#[tauri::command]
fn delete_history_item(state: State<'_, HistoryState>, id: u64) {
    state.delete(id);
}

#[tauri::command]
fn clear_history(state: State<'_, HistoryState>) {
    state.clear();
}

#[tauri::command]
async fn repaste_history_item(_clipboard_state: State<'_, ClipboardState>, text: String) -> Result<(), String> {
    if let Err(e) = clipboard::set_clipboard(&text) {
        return Err(format!("Failed to set clipboard: {}", e));
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
        tauri::WebviewUrl::App("index.html?view=settings".into())
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

#[tauri::command]
async fn start_recording_cmd(app: AppHandle, audio_state: State<'_, AudioState>) -> Result<(), String> {
    match audio::start_recording(&*audio_state).await {
        Ok(()) => {
            app.emit("status", "recording").unwrap();
            Ok(())
        }
        Err(e) => {
            eprintln!("[FamVoice] Failed to start recording: {}", e);
            app.emit("status", "error").unwrap();
            app.emit("transcript", e.clone()).unwrap();
            Err(e)
        }
    }
}

#[tauri::command]
async fn stop_recording_cmd(
    app: AppHandle,
    audio_state: State<'_, AudioState>,
    settings_state: State<'_, SettingsState>,
    history_state: State<'_, HistoryState>,
    clipboard_state: State<'_, ClipboardState>,
    http_state: State<'_, HttpClientState>,
) -> Result<(), String> {
    let t_total = std::time::Instant::now();
    app.emit("status", "transcribing").unwrap();
    let samples = match audio::stop_recording(&*audio_state).await {
        Some(s) => s,
        None => {
            eprintln!("[FamVoice] stop_recording returned None — was not recording");
            app.emit("status", "idle").unwrap();
            return Err("Not recording".into());
        }
    };

    // Calculate RMS volume to detect silence
    let mut sum_squares = 0.0;
    for &sample in &samples {
        let s = sample as f64;
        sum_squares += s * s;
    }
    let rms = (sum_squares / samples.len() as f64).sqrt();
    eprintln!("[FamVoice] Audio RMS volume: {:.2}", rms);

    if rms < 50.0 {
        eprintln!("[FamVoice] Silence detected, skipping transcription");
        app.emit("status", "error").unwrap();
        app.emit("transcript", "No voice detected").unwrap();
        
        let app_clone = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            let _ = app_clone.emit("status", "idle");
        });
        
        return Err("No voice detected".into());
    }

    // Encode to WAV in memory — no disk I/O
    let t_encode = std::time::Instant::now();
    let wav_bytes = audio::encode_wav_in_memory(&samples);
    eprintln!("[FamVoice] WAV encode: {} samples ({:.1}s) → {} bytes in {:.0}ms",
        samples.len(), samples.len() as f64 / 16000.0, wav_bytes.len(),
        t_encode.elapsed().as_secs_f64() * 1000.0);
    drop(samples);

    let settings = settings_state.settings.lock().unwrap().clone();
    if settings.api_key.is_empty() {
        eprintln!("[FamVoice] API key is empty!");
        app.emit("status", "error").unwrap();
        app.emit("transcript", "API key is empty. Set it in Settings.").unwrap();
        return Err("API key is empty".into());
    }

    eprintln!("[FamVoice] Transcribing with model: {}, language: {}", settings.model, settings.language);
    let lang = if settings.language == "auto" { None } else { Some(settings.language.as_str()) };

    let t_api = std::time::Instant::now();
    match transcription::transcribe_audio(&http_state.client, wav_bytes, &settings.api_key, &settings.model, lang).await {
        Ok(mut text) => {
            eprintln!("[FamVoice] API call: {:.0}ms | Total: {:.0}ms | Result ({} chars): {:?}",
                t_api.elapsed().as_secs_f64() * 1000.0,
                t_total.elapsed().as_secs_f64() * 1000.0,
                text.len(), &text[..text.len().min(100)]);
            
            // Apply replacements
            for rep in &settings.replacements {
                text = text.replace(&rep.target, &rep.replacement);
            }

            // Remove trailing dot if present
            if text.ends_with('.') {
                text.pop();
            }

            if !settings.preserve_clipboard {
                clipboard::save_clipboard(&*clipboard_state);
            }

            if let Err(e) = clipboard::set_clipboard(&text) {
                eprintln!("[FamVoice] Failed to set clipboard: {}", e);
            }

            let mut paste_successful = true;
            let mut paste_error = None;

            if settings.auto_paste {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                if let Err(e) = injection::simulate_paste() {
                    eprintln!("[FamVoice] Failed to simulate paste: {}", e);
                    paste_successful = false;
                    paste_error = Some(e);
                }
            }

            if !settings.preserve_clipboard && paste_successful {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                clipboard::restore_clipboard(&*clipboard_state);
            }

            history_state.add(text.clone());
            
            if !paste_successful {
                app.emit("status", "error").unwrap();
                let error_msg = format!("Paste failed: {}. Transcript is on clipboard.", paste_error.unwrap_or_default());
                app.emit("transcript", error_msg).unwrap();
            } else {
                app.emit("transcript", text).unwrap();
                app.emit("status", "success").unwrap();
            }
        }
        Err(e) => {
            eprintln!("[FamVoice] Transcription error: {}", e);
            app.emit("status", "error").unwrap();
            app.emit("transcript", e.clone()).unwrap();
            return Err(e);
        }
    }

    // Non-blocking status reset: returns immediately, resets after 2s in background
    let app_clone = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let _ = app_clone.emit("status", "idle");
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"])
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&app_dir).unwrap_or_default();

            app.manage(AudioState::default());
            app.manage(SettingsState::load(app_dir.clone()));
            app.manage(HistoryState::load(app_dir));
            app.manage(ClipboardState::default());
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

            app.manage(HttpClientState { client: http_client });

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
                    } | TrayIconEvent::DoubleClick {
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
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
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
                    }
                })
                .build(app)?;

            let hotkey_shared = Arc::new(Mutex::new(String::new()));
            app.manage(HotkeyConfigState { hotkey: hotkey_shared.clone() });

            // Register initial shortcut and start mouse listener
            let hotkey = {
                let state: State<SettingsState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings.hotkey.clone()
            };
            
            register_hotkey(app.handle(), &hotkey);
            input_hook::start_mouse_listener(app.handle().clone(), hotkey_shared);

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
            open_settings_window,
            close_settings_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}