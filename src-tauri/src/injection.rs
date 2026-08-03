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
fn trigger_paste_shortcut(enigo: &mut Enigo, modifier_delay: Duration) -> Result<(), String> {
    enigo
        .key(enigo::Key::Shift, enigo::Direction::Press)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
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
fn trigger_paste_shortcut(enigo: &mut Enigo, modifier_delay: Duration) -> Result<(), String> {
    let modifier_key = paste_modifier_key();

    enigo
        .key(modifier_key, enigo::Direction::Press)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    enigo
        .key(enigo::Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| e.to_string())?;
    sleep(modifier_delay);
    enigo
        .key(modifier_key, enigo::Direction::Release)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn simulate_paste() -> Result<(), String> {
    // enigo 0.6.1 uses new struct initialization
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Add small delays to ensure modifier keys are registered by the OS before the paste key is pressed.
    // This makes the text injection significantly more robust across different systems and load conditions.
    let modifier_delay = modifier_delay();
    trigger_paste_shortcut(&mut enigo, modifier_delay)
}

fn direct_text_segments(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::to_string)
        .collect()
}

trait DirectTextSink {
    fn write_text(&mut self, text: &str) -> Result<(), String>;
    fn press_return(&mut self) -> Result<(), String>;
}

impl DirectTextSink for Enigo {
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.text(text).map_err(|error| error.to_string())
    }

    fn press_return(&mut self) -> Result<(), String> {
        self.key(enigo::Key::Return, enigo::Direction::Click)
            .map_err(|error| error.to_string())
    }
}

fn simulate_text_with_sink(text: &str, sink: &mut dyn DirectTextSink) -> Result<(), String> {
    crate::delivery::validate_text_length(text)?;
    let segments = direct_text_segments(text);

    for (index, segment) in segments.iter().enumerate() {
        sink.write_text(segment)?;
        if index + 1 < segments.len() {
            sink.press_return()?;
        }
    }

    Ok(())
}

pub fn simulate_text(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    simulate_text_with_sink(text, &mut enigo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RejectingSink {
        text_calls: usize,
        return_calls: usize,
        reject_text_call: Option<usize>,
    }

    impl DirectTextSink for RejectingSink {
        fn write_text(&mut self, _text: &str) -> Result<(), String> {
            self.text_calls += 1;
            if self.reject_text_call == Some(self.text_calls) {
                return Err("simulated application rejection".to_string());
            }
            Ok(())
        }

        fn press_return(&mut self) -> Result<(), String> {
            self.return_calls += 1;
            Ok(())
        }
    }

    #[test]
    fn test_paste_injection_uses_short_modifier_delay() {
        assert_eq!(modifier_delay(), Duration::from_millis(4));
    }

    #[test]
    fn test_paste_shortcut_label_matches_target_platform() {
        #[cfg(target_os = "windows")]
        assert_eq!(paste_shortcut_label(), "Shift+Insert");
        #[cfg(target_os = "macos")]
        assert_eq!(paste_shortcut_label(), "Command+V");
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        assert_eq!(paste_shortcut_label(), "Control+V");
    }

    #[test]
    fn direct_text_segments_normalize_each_line_break_once() {
        assert_eq!(
            direct_text_segments("first\r\nsecond\rthird\nfourth"),
            vec!["first", "second", "third", "fourth"]
        );
        assert_eq!(
            direct_text_segments("first\n\nthird"),
            vec!["first", "", "third"]
        );
    }

    #[test]
    fn direct_text_insertion_preserves_unicode_and_multiline_order() {
        let mut sink = RejectingSink::default();

        simulate_text_with_sink("Olá 👋\n第二行\nthird", &mut sink).unwrap();

        assert_eq!(sink.text_calls, 3);
        assert_eq!(sink.return_calls, 2);
    }

    #[test]
    fn direct_text_insertion_stops_when_the_application_rejects_events() {
        let mut sink = RejectingSink {
            reject_text_call: Some(2),
            ..RejectingSink::default()
        };

        let error = simulate_text_with_sink("first\nsecond\nthird", &mut sink)
            .expect_err("the simulated application must reject the second line");

        assert_eq!(error, "simulated application rejection");
        assert_eq!(sink.text_calls, 2);
        assert_eq!(sink.return_calls, 1);
    }

    #[test]
    fn direct_text_insertion_rejects_oversized_text_before_sending_events() {
        let mut sink = RejectingSink::default();
        let oversized = "a".repeat(crate::delivery::MAX_DELIVERED_TEXT_CHARS + 1);

        let error = simulate_text_with_sink(&oversized, &mut sink)
            .expect_err("oversized direct insertion must be refused");

        assert!(error.contains("too long"));
        assert_eq!(sink.text_calls, 0);
        assert_eq!(sink.return_calls, 0);
    }
}
