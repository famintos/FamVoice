use crate::audio::AudioState;
#[cfg(not(target_os = "windows"))]
use rdev::grab;
#[cfg(target_os = "windows")]
use rdev::listen;
use rdev::{Button, Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

pub struct HotkeyConfigState {
    pub hotkey: Arc<Mutex<String>>,
}

static MOUSE_HOTKEY_PRESSED: AtomicBool = AtomicBool::new(false);
const MOUSE_GRAB_RETRY_INITIAL_DELAY_MS: u64 = 500;
const MOUSE_GRAB_RETRY_MAX_DELAY_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseHotkeyState {
    Idle,
    Pressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseHotkeyEvent {
    TargetPress,
    TargetRelease,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseHotkeyAction {
    PassThrough,
    Swallow,
    StartRecording,
    StopRecording,
}

pub fn is_mouse_hotkey(hotkey: &str) -> bool {
    hotkey.starts_with("Mouse")
}

pub fn reset_mouse_hotkey_state() {
    MOUSE_HOTKEY_PRESSED.store(false, Ordering::SeqCst);
}

fn parse_mouse_button(hotkey: &str) -> Option<Button> {
    match hotkey {
        "Mouse4" => {
            #[cfg(target_os = "windows")]
            {
                Some(Button::Unknown(1))
            }
            #[cfg(not(target_os = "windows"))]
            {
                Some(Button::Unknown(4))
            }
        }
        "Mouse5" => {
            #[cfg(target_os = "windows")]
            {
                Some(Button::Unknown(2))
            }
            #[cfg(not(target_os = "windows"))]
            {
                Some(Button::Unknown(5))
            }
        }
        "Mouse3" => Some(Button::Middle),
        _ => None,
    }
}

fn current_mouse_hotkey_state() -> MouseHotkeyState {
    if MOUSE_HOTKEY_PRESSED.load(Ordering::SeqCst) {
        MouseHotkeyState::Pressed
    } else {
        MouseHotkeyState::Idle
    }
}

fn set_mouse_hotkey_state(state: MouseHotkeyState) {
    MOUSE_HOTKEY_PRESSED.store(matches!(state, MouseHotkeyState::Pressed), Ordering::SeqCst);
}

fn decide_mouse_hotkey_action(
    state: MouseHotkeyState,
    event: MouseHotkeyEvent,
) -> (MouseHotkeyState, MouseHotkeyAction) {
    match (state, event) {
        (MouseHotkeyState::Idle, MouseHotkeyEvent::TargetPress) => {
            (MouseHotkeyState::Pressed, MouseHotkeyAction::StartRecording)
        }
        (MouseHotkeyState::Pressed, MouseHotkeyEvent::TargetRelease) => {
            (MouseHotkeyState::Idle, MouseHotkeyAction::StopRecording)
        }
        (MouseHotkeyState::Pressed, MouseHotkeyEvent::TargetPress)
        | (MouseHotkeyState::Idle, MouseHotkeyEvent::TargetRelease) => {
            (state, MouseHotkeyAction::Swallow)
        }
        (_, MouseHotkeyEvent::Other) => (state, MouseHotkeyAction::PassThrough),
    }
}

pub fn start_mouse_listener(app: AppHandle, hotkey_shared: Arc<Mutex<String>>) {
    thread::spawn(move || {
        let mut retry_delay = Duration::from_millis(MOUSE_GRAB_RETRY_INITIAL_DELAY_MS);

        loop {
            let app_handle = app.clone();
            let hotkey = hotkey_shared.clone();

            #[cfg(target_os = "windows")]
            let listener_result = listen(move |event| {
                handle_event_windows(&app_handle, &hotkey, event);
            });

            #[cfg(not(target_os = "windows"))]
            let listener_result = grab(move |event| handle_event(&app_handle, &hotkey, event));

            match listener_result {
                Ok(()) => {
                    eprintln!("[FamVoice] Mouse listener stopped unexpectedly, restarting");
                    retry_delay = Duration::from_millis(MOUSE_GRAB_RETRY_INITIAL_DELAY_MS);
                    thread::sleep(retry_delay);
                }
                Err(error) => {
                    eprintln!(
                        "[FamVoice] Mouse listener error: {:?}. Retrying in {}ms",
                        error,
                        retry_delay.as_millis()
                    );
                    thread::sleep(retry_delay);
                    let next_delay_ms = (retry_delay.as_millis() as u64)
                        .saturating_mul(2)
                        .min(MOUSE_GRAB_RETRY_MAX_DELAY_MS);
                    retry_delay = Duration::from_millis(next_delay_ms);
                }
            }
        }
    });
}

fn process_event(
    app: &AppHandle,
    hotkey_shared: &Arc<Mutex<String>>,
    event: &Event,
) -> MouseHotkeyAction {
    let current_hotkey = {
        let lock = hotkey_shared.lock().unwrap_or_else(|e| {
            eprintln!("[FamVoice] Hotkey config lock poisoned, recovering");
            e.into_inner()
        });
        lock.clone()
    };

    if !is_mouse_hotkey(&current_hotkey) {
        reset_mouse_hotkey_state();
        return MouseHotkeyAction::PassThrough;
    }

    let target_button = match parse_mouse_button(&current_hotkey) {
        Some(b) => b,
        None => return MouseHotkeyAction::PassThrough,
    };

    let hotkey_event = match event.event_type {
        EventType::ButtonPress(btn) if btn == target_button => MouseHotkeyEvent::TargetPress,
        EventType::ButtonRelease(btn) if btn == target_button => MouseHotkeyEvent::TargetRelease,
        EventType::ButtonPress(btn) => {
            if let Button::Unknown(code) = btn {
                eprintln!(
                    "[FamVoice] Mouse Button Press: Unknown({}) (Target: {:?})",
                    code, target_button
                );
            }
            MouseHotkeyEvent::Other
        }
        _ => MouseHotkeyEvent::Other,
    };

    let (next_state, action) =
        decide_mouse_hotkey_action(current_mouse_hotkey_state(), hotkey_event);
    set_mouse_hotkey_state(next_state);

    match action {
        MouseHotkeyAction::StartRecording => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let audio_state: State<AudioState> = app_clone.state();
                let is_recording = audio_state.is_recording.load(Ordering::SeqCst);
                if !is_recording {
                    let _ = crate::start_recording_cmd(app_clone.clone()).await;
                }
            });
        }
        MouseHotkeyAction::StopRecording => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let audio_state: State<AudioState> = app_clone.state();
                let is_recording = audio_state.is_recording.load(Ordering::SeqCst);
                if !is_recording {
                    return;
                }
                let _ = crate::stop_recording_cmd(app_clone.clone()).await;
            });
        }
        MouseHotkeyAction::Swallow | MouseHotkeyAction::PassThrough => {}
    }

    action
}

#[cfg(target_os = "windows")]
fn handle_event_windows(
    app: &AppHandle,
    hotkey_shared: &Arc<Mutex<String>>,
    event: Event,
) {
    let _ = process_event(app, hotkey_shared, &event);
}

#[cfg(not(target_os = "windows"))]
fn handle_event(
    app: &AppHandle,
    hotkey_shared: &Arc<Mutex<String>>,
    event: Event,
) -> Option<Event> {
    match process_event(app, hotkey_shared, &event) {
        MouseHotkeyAction::StartRecording
        | MouseHotkeyAction::StopRecording
        | MouseHotkeyAction::Swallow => None,
        MouseHotkeyAction::PassThrough => Some(event),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_press_starts_recording_and_arms_listener() {
        let (next_state, action) =
            decide_mouse_hotkey_action(MouseHotkeyState::Idle, MouseHotkeyEvent::TargetPress);

        assert_eq!(next_state, MouseHotkeyState::Pressed);
        assert_eq!(action, MouseHotkeyAction::StartRecording);
    }

    #[test]
    fn test_target_release_stops_recording_and_resets_listener() {
        let (next_state, action) =
            decide_mouse_hotkey_action(MouseHotkeyState::Pressed, MouseHotkeyEvent::TargetRelease);

        assert_eq!(next_state, MouseHotkeyState::Idle);
        assert_eq!(action, MouseHotkeyAction::StopRecording);
    }

    #[test]
    fn test_duplicate_target_press_is_swallowed_without_restarting() {
        let (next_state, action) =
            decide_mouse_hotkey_action(MouseHotkeyState::Pressed, MouseHotkeyEvent::TargetPress);

        assert_eq!(next_state, MouseHotkeyState::Pressed);
        assert_eq!(action, MouseHotkeyAction::Swallow);
    }

    #[test]
    fn test_non_target_events_pass_through() {
        let (next_state, action) =
            decide_mouse_hotkey_action(MouseHotkeyState::Idle, MouseHotkeyEvent::Other);

        assert_eq!(next_state, MouseHotkeyState::Idle);
        assert_eq!(action, MouseHotkeyAction::PassThrough);
    }
}
