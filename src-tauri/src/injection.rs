use enigo::{Enigo, Keyboard, Settings};
use std::time::Duration;
use std::thread::sleep;

pub fn simulate_paste() -> Result<(), String> {
    // enigo 0.6.1 uses new struct initialization
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    
    // Add small delays to ensure modifier keys are registered by the OS before the 'v' key is pressed.
    // This makes the text injection significantly more robust across different systems and load conditions.
    let modifier_delay = Duration::from_millis(10);

    #[cfg(target_os = "macos")]
    {
        enigo.key(enigo::Key::Meta, enigo::Direction::Press).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Unicode('v'), enigo::Direction::Click).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Meta, enigo::Direction::Release).map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "windows")]
    {
        enigo.key(enigo::Key::Control, enigo::Direction::Press).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Unicode('v'), enigo::Direction::Click).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Control, enigo::Direction::Release).map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "linux")]
    {
        enigo.key(enigo::Key::Control, enigo::Direction::Press).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Unicode('v'), enigo::Direction::Click).map_err(|e| e.to_string())?;
        sleep(modifier_delay);
        enigo.key(enigo::Key::Control, enigo::Direction::Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}
