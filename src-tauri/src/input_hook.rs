use rdev::{grab, Event, EventType, Button};
use tauri::{AppHandle, Manager, State};
use std::sync::{Arc, Mutex};
use std::thread;
use crate::audio::AudioState;
use crate::settings::SettingsState;
use crate::history::HistoryState;
use crate::clipboard::ClipboardState;
use crate::HttpClientState;

pub struct HotkeyConfigState {
    pub hotkey: Arc<Mutex<String>>,
}

pub fn is_mouse_hotkey(hotkey: &str) -> bool {
    hotkey.starts_with("Mouse")
}

fn parse_mouse_button(hotkey: &str) -> Option<Button> {
    match hotkey {
        "Mouse4" => {
            #[cfg(target_os = "windows")]
            { Some(Button::Unknown(1)) }
            #[cfg(not(target_os = "windows"))]
            { Some(Button::Unknown(4)) }
        }
        "Mouse5" => {
            #[cfg(target_os = "windows")]
            { Some(Button::Unknown(2)) }
            #[cfg(not(target_os = "windows"))]
            { Some(Button::Unknown(5)) }
        }
        "Mouse3" => Some(Button::Middle),
        _ => None,
    }
}

pub fn start_mouse_listener(app: AppHandle, hotkey_shared: Arc<Mutex<String>>) {
    thread::spawn(move || {
        if let Err(error) = grab(move |event| {
            handle_event(&app, &hotkey_shared, event)
        }) {
            eprintln!("[FamVoice] Mouse grabber error: {:?}", error);
        }
    });
}

fn handle_event(app: &AppHandle, hotkey_shared: &Arc<Mutex<String>>, event: Event) -> Option<Event> {
    let current_hotkey = {
        let lock = hotkey_shared.lock().unwrap();
        lock.clone()
    };

    if !is_mouse_hotkey(&current_hotkey) {
        return Some(event);
    }

    let target_button = match parse_mouse_button(&current_hotkey) {
        Some(b) => b,
        None => return Some(event),
    };

    let mut should_swallow = false;

    match event.event_type {
        EventType::ButtonPress(btn) if btn == target_button => {
            should_swallow = true;
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let audio_state: State<AudioState> = app_clone.state();
                let is_recording = audio_state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                if !is_recording {
                    let _ = crate::start_recording_cmd(app_clone.clone(), audio_state).await;
                }
            });
        }
        EventType::ButtonPress(btn) => {
            // Diagnostic logging for unknown buttons to help users identify their codes
            if let Button::Unknown(code) = btn {
                eprintln!("[FamVoice] Mouse Button Press: Unknown({}) (Target: {:?})", code, target_button);
            }
        }
        EventType::ButtonRelease(btn) if btn == target_button => {
            should_swallow = true;
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let audio_state: State<AudioState> = app_clone.state();
                let settings_state: State<SettingsState> = app_clone.state();
                let history_state: State<HistoryState> = app_clone.state();
                let clipboard_state: State<ClipboardState> = app_clone.state();
                let http_state: State<HttpClientState> = app_clone.state();
                let _ = crate::stop_recording_cmd(
                    app_clone.clone(),
                    audio_state,
                    settings_state,
                    history_state,
                    clipboard_state,
                    http_state
                ).await;
            });
        }
        _ => {}
    }

    if should_swallow {
        None
    } else {
        Some(event)
    }
}
