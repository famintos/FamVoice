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
    match Clipboard::new() {
        Ok(mut clipboard) => match clipboard.get_text() {
            Ok(text) => {
                if let Ok(mut saved) = state.saved_text.lock() {
                    *saved = Some(text);
                }
            }
            Err(e) => {
                eprintln!("[FamVoice] Failed to read clipboard: {}", e);
                if let Ok(mut saved) = state.saved_text.lock() {
                    *saved = None;
                }
            }
        },
        Err(e) => {
            eprintln!("[FamVoice] Failed to open clipboard: {}", e);
        }
    }
}

pub fn saved_clipboard_text(state: &ClipboardState) -> Option<String> {
    state.saved_text.lock().ok().and_then(|saved| saved.clone())
}

pub fn restore_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
}

pub fn set_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())
}
