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

#[cfg(test)]
mod tests {
    use super::*;

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
}
