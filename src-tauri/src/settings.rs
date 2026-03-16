use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize)]
pub struct Replacement {
    pub target: String,
    pub replacement: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_key: String,
    pub model: String,
    pub language: String,
    pub auto_paste: bool,
    pub preserve_clipboard: bool,
    pub hotkey: String,
    pub widget_mode: bool,
    pub replacements: Vec<Replacement>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            model: "gpt-4o-mini-transcribe".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: true,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            widget_mode: false,
            replacements: Vec::new(),
        }
    }
}

pub struct SettingsState {
    pub settings: Mutex<AppSettings>,
    pub path: PathBuf,
}

impl SettingsState {
    pub fn load(app_dir: PathBuf) -> Self {
        let path = app_dir.join("settings.json");
        let settings = if path.exists() {
            let data = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            AppSettings::default()
        };
        Self {
            settings: Mutex::new(settings),
            path,
        }
    }

    pub fn save(&self) {
        if let Ok(settings) = self.settings.lock() {
            if let Ok(data) = serde_json::to_string_pretty(&*settings) {
                let _ = fs::write(&self.path, data);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_settings_load_save() {
        let dir = tempdir().unwrap();
        let state = SettingsState::load(dir.path().to_path_buf());
        
        {
            let mut settings = state.settings.lock().unwrap();
            settings.api_key = "test_key".to_string();
            settings.replacements.push(Replacement {
                target: "hello".to_string(),
                replacement: "world".to_string(),
            });
        }
        
        state.save();
        
        let new_state = SettingsState::load(dir.path().to_path_buf());
        let settings = new_state.settings.lock().unwrap();
        
        assert_eq!(settings.api_key, "test_key");
        assert_eq!(settings.replacements.len(), 1);
        assert_eq!(settings.replacements[0].target, "hello");
        assert_eq!(settings.replacements[0].replacement, "world");
    }
}