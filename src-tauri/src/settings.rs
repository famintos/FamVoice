use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub const SUPPORTED_MODELS: [&str; 3] = [
    "gpt-4o-mini-transcribe",
    "gpt-4o-transcribe",
    "whisper-1",
];

pub const SUPPORTED_LANGUAGE_PREFERENCES: [&str; 3] = ["auto", "pt", "en"];
pub const MIN_MIC_SENSITIVITY: u8 = 0;
pub const MAX_MIC_SENSITIVITY: u8 = 100;
pub const DEFAULT_MIC_SENSITIVITY: u8 = 60;

#[derive(Clone, Serialize, Deserialize)]
pub struct Replacement {
    pub target: String,
    pub replacement: String,
}

fn default_mic_sensitivity() -> u8 {
    DEFAULT_MIC_SENSITIVITY
}

fn normalize_language_preference(language: &str) -> String {
    match language {
        "auto" => "auto".to_string(),
        "pt" | "pt-first" => "pt".to_string(),
        "en" | "en-first" => "en".to_string(),
        _ => "auto".to_string(),
    }
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
    #[serde(default = "default_mic_sensitivity")]
    pub mic_sensitivity: u8,
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
            mic_sensitivity: default_mic_sensitivity(),
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
            match fs::read_to_string(&path) {
                Ok(data) => match serde_json::from_str::<AppSettings>(&data) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "[FamVoice] Failed to parse settings.json: {}, creating backup",
                            e
                        );
                        let backup_path = app_dir.join("settings.json.corrupt");
                        let _ = fs::copy(&path, &backup_path);
                        AppSettings::default()
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[FamVoice] Failed to read settings.json: {}, creating backup",
                        e
                    );
                    let backup_path = app_dir.join("settings.json.corrupt");
                    let _ = fs::copy(&path, &backup_path);
                    AppSettings::default()
                }
            }
        } else {
            AppSettings::default()
        };
        let settings = AppSettings {
            language: normalize_language_preference(&settings.language),
            ..settings
        };

        Self {
            settings: Mutex::new(settings),
            path,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Ok(settings) = self.settings.lock() {
            if let Ok(data) = serde_json::to_string_pretty(&*settings) {
                let backup_path = self.path.with_extension("json.bak");
                let _ = fs::copy(&self.path, &backup_path);
                fs::write(&self.path, data).map_err(|e| {
                    eprintln!("[FamVoice] Failed to save settings: {}", e);
                    let _ = fs::copy(&backup_path, &self.path);
                    format!("Failed to save settings: {}", e)
                })
            } else {
                Err("Failed to serialize settings".to_string())
            }
        } else {
            Err("Failed to lock settings".to_string())
        }
    }
}

pub fn validate_settings(settings: &AppSettings) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if settings.api_key.len() > 200 {
        errors.push("API key is too long".to_string());
    }

    if !SUPPORTED_MODELS.contains(&settings.model.as_str()) {
        errors.push(format!(
            "Unsupported model: {}. Use one of: {}",
            settings.model
                ,
            SUPPORTED_MODELS.join(", ")
        ));
    }

    if !SUPPORTED_LANGUAGE_PREFERENCES.contains(&settings.language.as_str()) {
        errors.push(format!(
            "Invalid language: {}. Use one of: {}",
            settings.language
                ,
            SUPPORTED_LANGUAGE_PREFERENCES.join(", ")
        ));
    }

    if settings.hotkey.len() > 100 {
        errors.push("Hotkey is too long".to_string());
    }

    if !(MIN_MIC_SENSITIVITY..=MAX_MIC_SENSITIVITY).contains(&settings.mic_sensitivity) {
        errors.push(format!(
            "Mic sensitivity must be between {} and {}",
            MIN_MIC_SENSITIVITY, MAX_MIC_SENSITIVITY
        ));
    }

    for (i, rep) in settings.replacements.iter().enumerate() {
        if rep.target.trim().is_empty() {
            errors.push(format!("Replacement {} target cannot be empty", i + 1));
        }
        if rep.target.len() > 100 {
            errors.push(format!(
                "Replacement {} target is too long (max 100 chars)",
                i + 1
            ));
        }
        if rep.replacement.len() > 100 {
            errors.push(format!(
                "Replacement {} replacement is too long (max 100 chars)",
                i + 1
            ));
        }
    }

    if settings.replacements.len() > 50 {
        errors.push("Too many replacements (max 50)".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_settings() -> AppSettings {
        AppSettings {
            api_key: "sk-test".to_string(),
            model: "gpt-4o-mini-transcribe".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: false,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            widget_mode: false,
            mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
            replacements: vec![],
        }
    }

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

        state.save().ok();

        let new_state = SettingsState::load(dir.path().to_path_buf());
        let settings = new_state.settings.lock().unwrap();

        assert_eq!(settings.api_key, "test_key");
        assert_eq!(settings.replacements.len(), 1);
        assert_eq!(settings.replacements[0].target, "hello");
        assert_eq!(settings.replacements[0].replacement, "world");
    }

    #[test]
    fn test_validate_settings_valid() {
        let settings = sample_settings();
        assert!(validate_settings(&settings).is_ok());
    }

    #[test]
    fn test_validate_settings_invalid_model() {
        let settings = AppSettings {
            model: "invalid-model".to_string(),
            ..sample_settings()
        };
        let result = validate_settings(&settings);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("Unsupported model")));
    }

    #[test]
    fn test_validate_settings_invalid_language() {
        let settings = AppSettings {
            language: "invalid".to_string(),
            ..sample_settings()
        };
        let result = validate_settings(&settings);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("Invalid language")));
    }

    #[test]
    fn test_validate_settings_replacement_too_long() {
        let settings = AppSettings {
            replacements: vec![Replacement {
                target: "a".repeat(101),
                replacement: "b".to_string(),
            }],
            ..sample_settings()
        };
        let result = validate_settings(&settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|e| e.contains("too long")));
    }

    #[test]
    fn test_validate_settings_accepts_language_preferences() {
        for language in ["auto", "pt", "en"] {
            let settings = AppSettings {
                language: language.to_string(),
                ..sample_settings()
            };
            assert!(
                validate_settings(&settings).is_ok(),
                "expected language {language} to be accepted"
            );
        }
    }

    #[test]
    fn test_validate_settings_rejects_blank_replacement_target() {
        let settings = AppSettings {
            replacements: vec![Replacement {
                target: "   ".to_string(),
                replacement: "hello".to_string(),
            }],
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("target cannot be empty")));
    }

    #[test]
    fn test_default_settings_include_mic_sensitivity() {
        let settings = AppSettings::default();

        assert_eq!(settings.mic_sensitivity, 60);
    }

    #[test]
    fn test_settings_loads_default_mic_sensitivity_from_legacy_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "api_key": "sk-test",
  "model": "gpt-4o-mini-transcribe",
  "language": "en",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "replacements": []
}"#,
        )
        .unwrap();

        let state = SettingsState::load(dir.path().to_path_buf());
        let settings = state.settings.lock().unwrap();

        assert_eq!(settings.mic_sensitivity, 60);
        assert_eq!(settings.language, "en");
    }

    #[test]
    fn test_settings_load_normalizes_legacy_portuguese_language_preference() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "api_key": "sk-test",
  "model": "gpt-4o-mini-transcribe",
  "language": "pt",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "replacements": []
}"#,
        )
        .unwrap();

        let state = SettingsState::load(dir.path().to_path_buf());
        let settings = state.settings.lock().unwrap();

        assert_eq!(settings.language, "pt");
    }

    #[test]
    fn test_settings_load_normalizes_legacy_portuguese_first_language_preference() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "api_key": "sk-test",
  "model": "gpt-4o-mini-transcribe",
  "language": "pt-first",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "replacements": []
}"#,
        )
        .unwrap();

        let state = SettingsState::load(dir.path().to_path_buf());
        let settings = state.settings.lock().unwrap();

        assert_eq!(settings.language, "pt");
    }

    #[test]
    fn test_settings_load_normalizes_legacy_english_first_language_preference() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "api_key": "sk-test",
  "model": "gpt-4o-mini-transcribe",
  "language": "en-first",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "replacements": []
}"#,
        )
        .unwrap();

        let state = SettingsState::load(dir.path().to_path_buf());
        let settings = state.settings.lock().unwrap();

        assert_eq!(settings.language, "en");
    }

    #[test]
    fn test_validate_settings_rejects_out_of_range_mic_sensitivity() {
        let settings = AppSettings {
            mic_sensitivity: 101,
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("Mic sensitivity")));
    }
}
