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

#[cfg(not(target_os = "macos"))]
fn paste_modifier_key() -> enigo::Key {
    enigo::Key::Control
}

pub fn simulate_paste() -> Result<(), String> {
    // enigo 0.6.1 uses new struct initialization
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Add small delays to ensure modifier keys are registered by the OS before the 'v' key is pressed.
    // This makes the text injection significantly more robust across different systems and load conditions.
    let modifier_delay = modifier_delay();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paste_injection_uses_short_modifier_delay() {
        assert_eq!(modifier_delay(), Duration::from_millis(4));
    }
}
