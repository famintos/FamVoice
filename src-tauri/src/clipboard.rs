use arboard::Clipboard;
use std::sync::Mutex;

pub struct ClipboardState {
    saved_text: Mutex<Option<String>>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            saved_text: Mutex::new(None),
        }
    }
}

pub fn save_clipboard(state: &ClipboardState) {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            *state.saved_text.lock().unwrap() = Some(text);
        } else {
            *state.saved_text.lock().unwrap() = None;
        }
    }
}

pub fn restore_clipboard(state: &ClipboardState) {
    if let Some(text) = state.saved_text.lock().unwrap().clone() {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }
}

pub fn set_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
}