use enigo::{Enigo, Keyboard, Settings};
use std::thread::sleep;
use std::time::Duration;

const MODIFIER_DELAY_MS: u64 = 4;

pub fn modifier_delay() -> Duration {
    Duration::from_millis(MODIFIER_DELAY_MS)
}

#[cfg(target_os = "macos")]
fn paste_modifier_key() -> enigo::Key {
    enigo::Key::Meta
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn paste_modifier_key() -> enigo::Key {
    enigo::Key::Control
}

#[cfg(test)]
fn paste_shortcut_label() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Shift+Insert"
    }
    #[cfg(target_os = "macos")]
    {
        "Command+V"
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "Control+V"
    }
}

#[cfg(target_os = "windows")]
fn trigger_paste_shortcut(
    enigo: &mut Enigo,
    modifier_delay: Duration,
    target: Option<ForegroundTarget>,
) -> Result<(), String> {
    enigo
        .key(enigo::Key::Shift, enigo::Direction::Press)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    if let Err(error) = ensure_foreground_target(target) {
        let _ = enigo.key(enigo::Key::Shift, enigo::Direction::Release);
        return Err(error);
    }
    enigo
        .key(enigo::Key::Insert, enigo::Direction::Click)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    enigo
        .key(enigo::Key::Shift, enigo::Direction::Release)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn trigger_paste_shortcut(
    enigo: &mut Enigo,
    modifier_delay: Duration,
    target: Option<ForegroundTarget>,
) -> Result<(), String> {
    let modifier_key = paste_modifier_key();

    enigo
        .key(modifier_key, enigo::Direction::Press)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    if let Err(error) = ensure_foreground_target(target) {
        let _ = enigo.key(modifier_key, enigo::Direction::Release);
        return Err(error);
    }
    enigo
        .key(enigo::Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    enigo
        .key(modifier_key, enigo::Direction::Release)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn simulate_paste(target: Option<ForegroundTarget>) -> Result<(), String> {
    // enigo 0.6.1 uses new struct initialization
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Add small delays to ensure modifier keys are registered by the OS before the paste key is pressed.
    // This makes the text injection significantly more robust across different systems and load conditions.
    let modifier_delay = modifier_delay();
    ensure_foreground_target(target)?;
    trigger_paste_shortcut(&mut enigo, modifier_delay, target)
}

/// Single-line final text for both clipboard and direct injection.
pub fn normalize_final_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

trait DirectTextSink {
    fn write_text(&mut self, text: &str) -> Result<(), String>;
}
impl DirectTextSink for Enigo {
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.text(text).map_err(|error| error.to_string())
    }
}
fn simulate_text_with_sink(text: &str, sink: &mut dyn DirectTextSink) -> Result<(), String> {
    crate::delivery::validate_text_length(text)?;
    let text = normalize_final_text(text);
    let text = crate::transcription::validate_transcript_text(&text)?;
    sink.write_text(&text)
}

pub fn simulate_text(text: &str, target: Option<ForegroundTarget>) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    ensure_foreground_target(target)?;
    simulate_text_with_sink(text, &mut enigo)
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ForegroundTarget {
    window: usize,
    process: u32,
}

pub fn capture_foreground_target() -> Option<ForegroundTarget> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }
        let mut process = 0;
        GetWindowThreadProcessId(window, &mut process);
        if process == 0 {
            return None;
        }
        Some(ForegroundTarget {
            window: window as usize,
            process,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn target_matches(original: Option<ForegroundTarget>, current: Option<ForegroundTarget>) -> bool {
    original
        .is_some_and(|target| target.window != 0 && target.process != 0 && Some(target) == current)
}

pub fn ensure_foreground_target(target: Option<ForegroundTarget>) -> Result<(), String> {
    if target_matches(target, capture_foreground_target()) {
        Ok(())
    } else {
        Err("Insertion cancelled because the original foreground window is no longer active".into())
    }
}

#[derive(Default)]
pub struct DictationTargetState {
    recording: std::sync::Mutex<Option<(crate::dictation::SessionId, Option<ForegroundTarget>)>>,
    retry: std::sync::Mutex<Option<ForegroundTarget>>,
}

impl DictationTargetState {
    pub fn start(&self, id: crate::dictation::SessionId, target: Option<ForegroundTarget>) {
        if let Ok(mut recording) = self.recording.lock() {
            *recording = Some((id, target));
        }
        if let Ok(mut retry) = self.retry.lock() {
            *retry = None;
        }
    }
    pub fn take(&self, id: crate::dictation::SessionId) -> Option<ForegroundTarget> {
        let mut recording = self.recording.lock().ok()?;
        if recording
            .as_ref()
            .is_some_and(|(session, _)| *session == id)
        {
            recording.take().and_then(|(_, target)| target)
        } else {
            None
        }
    }
    pub fn remember_failed(&self, target: Option<ForegroundTarget>) {
        if let Ok(mut retry) = self.retry.lock() {
            *retry = target;
        }
    }
    pub fn take_retry(&self) -> Option<ForegroundTarget> {
        self.retry.lock().ok()?.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RejectingSink {
        received: Vec<String>,
        reject: bool,
    }
    impl DirectTextSink for RejectingSink {
        fn write_text(&mut self, text: &str) -> Result<(), String> {
            self.received.push(text.to_string());
            if self.reject {
                Err("simulated application rejection".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn no_submit_normalizes_controls_and_direct_route_uses_one_text_event() {
        let input = " Olá 👋\r\n第二行\rthird\n\tfourth\0\u{1b}\u{85}\u{2028}\u{2029}fim ";
        let safe = normalize_final_text(input);
        assert_eq!(safe, "Olá 👋 第二行 third fourth fim");
        assert!(!safe.chars().any(char::is_control));
        assert_eq!(normalize_final_text(&safe), safe);
        let mut sink = RejectingSink::default();
        simulate_text_with_sink(input, &mut sink).unwrap();
        assert_eq!(sink.received, vec![safe]);
    }

    #[test]
    fn no_submit_direct_route_reports_rejection_and_rejects_empty_or_oversized_text() {
        let mut sink = RejectingSink {
            reject: true,
            ..Default::default()
        };
        assert_eq!(
            simulate_text_with_sink("first\nsecond", &mut sink).unwrap_err(),
            "simulated application rejection"
        );
        assert_eq!(sink.received, vec!["first second"]);
        let mut sink = RejectingSink::default();
        assert!(simulate_text_with_sink("\r\n\0\t", &mut sink).is_err());
        assert!(simulate_text_with_sink(
            &"a".repeat(crate::delivery::MAX_DELIVERED_TEXT_CHARS + 1),
            &mut sink
        )
        .is_err());
        assert!(sink.received.is_empty());
    }

    #[test]
    fn test_paste_injection_uses_short_modifier_delay() {
        assert_eq!(modifier_delay(), Duration::from_millis(4));
        assert_eq!(paste_shortcut_label(), "Shift+Insert");
    }

    #[test]
    fn target_guard_fails_closed_for_missing_changed_or_reused_windows() {
        let original = ForegroundTarget {
            window: 42,
            process: 7,
        };
        assert!(target_matches(Some(original), Some(original)));
        assert!(!target_matches(None, Some(original)));
        assert!(!target_matches(Some(original), None));
        assert!(!target_matches(
            Some(original),
            Some(ForegroundTarget {
                window: 43,
                process: 7
            })
        ));
        assert!(!target_matches(
            Some(original),
            Some(ForegroundTarget {
                window: 42,
                process: 8
            })
        ));
        assert!(!target_matches(
            Some(ForegroundTarget::default()),
            Some(ForegroundTarget::default())
        ));
    }

    #[test]
    fn target_guard_keeps_session_and_retry_targets_without_retargeting() {
        let state = DictationTargetState::default();
        let target = ForegroundTarget {
            window: 42,
            process: 7,
        };
        state.start(1, Some(target));
        assert_eq!(state.take(2), None);
        assert_eq!(state.take(1), Some(target));
        assert_eq!(state.take(1), None);
        state.remember_failed(Some(target));
        assert_eq!(state.take_retry(), Some(target));
        assert_eq!(state.take_retry(), None);
        state.remember_failed(Some(target));
        state.start(2, None);
        assert_eq!(state.take_retry(), None);
        assert_eq!(state.take(2), None);
    }
}
