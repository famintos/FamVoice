use arboard::Clipboard;
use std::sync::Mutex;

pub struct ClipboardState {
    clipboard: Mutex<Option<Clipboard>>,
    saved_text: Mutex<Option<String>>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            clipboard: Mutex::new(None),
            saved_text: Mutex::new(None),
        }
    }
}

fn with_clipboard<T>(
    state: &ClipboardState,
    action: impl FnOnce(&mut Clipboard) -> Result<T, String>,
) -> Result<T, String> {
    let mut clipboard_guard = state
        .clipboard
        .lock()
        .map_err(|_| "Clipboard mutex poisoned".to_string())?;

    if clipboard_guard.is_none() {
        *clipboard_guard = Some(Clipboard::new().map_err(|e| e.to_string())?);
    }

    let clipboard = clipboard_guard
        .as_mut()
        .expect("clipboard should be initialized");
    action(clipboard)
}

pub fn save_clipboard(state: &ClipboardState) {
    match with_clipboard(state, |clipboard| {
        clipboard.get_text().map_err(|e| e.to_string())
    }) {
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
    }
}

pub fn saved_clipboard_text(state: &ClipboardState) -> Option<String> {
    state.saved_text.lock().ok().and_then(|saved| saved.clone())
}

pub fn restore_clipboard_text(state: &ClipboardState, text: &str) -> Result<(), String> {
    with_clipboard(state, |clipboard| {
        clipboard
            .set_text(text.to_string())
            .map_err(|e| e.to_string())
    })
}

pub fn set_clipboard(state: &ClipboardState, text: &str) -> Result<(), String> {
    restore_clipboard_text(state, text)
}
