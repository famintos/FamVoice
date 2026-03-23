use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const SETTINGS_SERVICE_NAME: &str = "com.famvoice.app";
const OPENAI_API_KEY_ACCOUNT: &str = "openai_api_key";
const GROQ_API_KEY_ACCOUNT: &str = "groq_api_key";
const ANTHROPIC_API_KEY_ACCOUNT: &str = "anthropic_api_key";
const MAX_API_KEY_LEN: usize = 200;
pub const SUPPORTED_PROVIDERS: [&str; 2] = ["openai", "groq"];
pub const OPENAI_MODELS: [&str; 3] =
    ["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"];
pub const GROQ_MODELS: [&str; 1] = ["whisper-large-v3-turbo"];
pub const SUPPORTED_LANGUAGE_PREFERENCES: [&str; 3] = ["auto", "pt", "en"];
pub const MIN_MIC_SENSITIVITY: u8 = 0;
pub const MAX_MIC_SENSITIVITY: u8 = 100;
pub const DEFAULT_MIC_SENSITIVITY: u8 = 60;
pub const MAX_ANTHROPIC_API_KEY_LEN: usize = 200;

#[derive(Clone, Serialize, Deserialize)]
pub struct Replacement {
    pub target: String,
    pub replacement: String,
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_model() -> String {
    OPENAI_MODELS[0].to_string()
}

fn models_for_provider(provider: &str) -> &'static [&'static str] {
    match provider {
        "groq" => &GROQ_MODELS,
        _ => &OPENAI_MODELS,
    }
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_auto_paste() -> bool {
    true
}

fn default_preserve_clipboard() -> bool {
    true
}

fn default_hotkey() -> String {
    "CommandOrControl+Shift+Space".to_string()
}

fn default_widget_mode() -> bool {
    false
}

fn default_mic_sensitivity() -> u8 {
    DEFAULT_MIC_SENSITIVITY
}

fn default_prompt_optimization_enabled() -> bool {
    false
}

fn default_prompt_optimizer_model() -> String {
    crate::prompt_optimizer::SUPPORTED_MODELS[0].to_string()
}

fn normalize_language_preference(language: &str) -> String {
    match language {
        "auto" => "auto".to_string(),
        "pt" | "pt-first" => "pt".to_string(),
        "en" | "en-first" => "en".to_string(),
        _ => "auto".to_string(),
    }
}

fn default_replacements() -> Vec<Replacement> {
    Vec::new()
}

fn mask_secret(secret: &str) -> Option<String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return None;
    }

    let prefix: String = trimmed.chars().take(3).collect();
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Some(format!("{prefix}...{suffix}"))
}

#[derive(Clone)]
pub struct AppSettings {
    pub transcription_provider: String,
    pub api_key: String,
    pub groq_api_key: String,
    pub model: String,
    pub language: String,
    pub auto_paste: bool,
    pub preserve_clipboard: bool,
    pub hotkey: String,
    pub widget_mode: bool,
    pub mic_sensitivity: u8,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
    pub anthropic_api_key: String,
    pub replacements: Vec<Replacement>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            transcription_provider: default_provider(),
            api_key: String::new(),
            groq_api_key: String::new(),
            model: default_model(),
            language: default_language(),
            auto_paste: default_auto_paste(),
            preserve_clipboard: default_preserve_clipboard(),
            hotkey: default_hotkey(),
            widget_mode: default_widget_mode(),
            mic_sensitivity: default_mic_sensitivity(),
            prompt_optimization_enabled: default_prompt_optimization_enabled(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            anthropic_api_key: String::new(),
            replacements: default_replacements(),
        }
    }
}

impl AppSettings {
    pub fn to_frontend(&self) -> FrontendSettings {
        FrontendSettings {
            transcription_provider: self.transcription_provider.clone(),
            api_key_present: !self.api_key.trim().is_empty(),
            api_key_masked: mask_secret(&self.api_key),
            groq_api_key_present: !self.groq_api_key.trim().is_empty(),
            groq_api_key_masked: mask_secret(&self.groq_api_key),
            model: self.model.clone(),
            language: self.language.clone(),
            auto_paste: self.auto_paste,
            preserve_clipboard: self.preserve_clipboard,
            hotkey: self.hotkey.clone(),
            widget_mode: self.widget_mode,
            mic_sensitivity: self.mic_sensitivity,
            prompt_optimization_enabled: self.prompt_optimization_enabled,
            prompt_optimizer_model: self.prompt_optimizer_model.clone(),
            anthropic_api_key_present: !self.anthropic_api_key.trim().is_empty(),
            anthropic_api_key_masked: mask_secret(&self.anthropic_api_key),
            replacements: self.replacements.clone(),
        }
    }

    pub fn transcription_api_key(&self) -> &str {
        match self.transcription_provider.as_str() {
            "groq" => &self.groq_api_key,
            _ => &self.api_key,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct FrontendSettings {
    pub transcription_provider: String,
    pub api_key_present: bool,
    pub api_key_masked: Option<String>,
    pub groq_api_key_present: bool,
    pub groq_api_key_masked: Option<String>,
    pub model: String,
    pub language: String,
    pub auto_paste: bool,
    pub preserve_clipboard: bool,
    pub hotkey: String,
    pub widget_mode: bool,
    pub mic_sensitivity: u8,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
    pub anthropic_api_key_present: bool,
    pub anthropic_api_key_masked: Option<String>,
    pub replacements: Vec<Replacement>,
}

#[derive(Clone, Deserialize)]
pub struct SaveSettingsRequest {
    pub transcription_provider: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub groq_api_key: Option<String>,
    pub model: String,
    pub language: String,
    pub auto_paste: bool,
    pub preserve_clipboard: bool,
    pub hotkey: String,
    pub widget_mode: bool,
    pub mic_sensitivity: u8,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    pub replacements: Vec<Replacement>,
}

impl SaveSettingsRequest {
    fn merge_with_existing(self, existing: &AppSettings) -> AppSettings {
        fn keep_existing_or_new(value: Option<String>, existing: &str) -> String {
            match value {
                Some(value) if value.trim().is_empty() => existing.to_string(),
                Some(value) => value,
                None => existing.to_string(),
            }
        }

        AppSettings {
            transcription_provider: self.transcription_provider,
            api_key: keep_existing_or_new(self.api_key, &existing.api_key),
            groq_api_key: keep_existing_or_new(self.groq_api_key, &existing.groq_api_key),
            model: self.model,
            language: self.language,
            auto_paste: self.auto_paste,
            preserve_clipboard: self.preserve_clipboard,
            hotkey: self.hotkey,
            widget_mode: self.widget_mode,
            mic_sensitivity: self.mic_sensitivity,
            prompt_optimization_enabled: self.prompt_optimization_enabled,
            prompt_optimizer_model: self.prompt_optimizer_model,
            anthropic_api_key: keep_existing_or_new(self.anthropic_api_key, &existing.anthropic_api_key),
            replacements: self.replacements,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct DiskSettings {
    #[serde(default = "default_provider")]
    transcription_provider: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    groq_api_key: Option<String>,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_auto_paste")]
    auto_paste: bool,
    #[serde(default = "default_preserve_clipboard")]
    preserve_clipboard: bool,
    #[serde(default = "default_hotkey")]
    hotkey: String,
    #[serde(default = "default_widget_mode")]
    widget_mode: bool,
    #[serde(default = "default_mic_sensitivity")]
    mic_sensitivity: u8,
    #[serde(default = "default_prompt_optimization_enabled")]
    prompt_optimization_enabled: bool,
    #[serde(default = "default_prompt_optimizer_model")]
    prompt_optimizer_model: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    anthropic_api_key: Option<String>,
    #[serde(default = "default_replacements")]
    replacements: Vec<Replacement>,
}

impl Default for DiskSettings {
    fn default() -> Self {
        Self {
            transcription_provider: default_provider(),
            api_key: None,
            groq_api_key: None,
            model: default_model(),
            language: default_language(),
            auto_paste: default_auto_paste(),
            preserve_clipboard: default_preserve_clipboard(),
            hotkey: default_hotkey(),
            widget_mode: default_widget_mode(),
            mic_sensitivity: default_mic_sensitivity(),
            prompt_optimization_enabled: default_prompt_optimization_enabled(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            anthropic_api_key: None,
            replacements: default_replacements(),
        }
    }
}

impl From<&AppSettings> for DiskSettings {
    fn from(settings: &AppSettings) -> Self {
        Self {
            transcription_provider: settings.transcription_provider.clone(),
            api_key: None,
            groq_api_key: None,
            model: settings.model.clone(),
            language: settings.language.clone(),
            auto_paste: settings.auto_paste,
            preserve_clipboard: settings.preserve_clipboard,
            hotkey: settings.hotkey.clone(),
            widget_mode: settings.widget_mode,
            mic_sensitivity: settings.mic_sensitivity,
            prompt_optimization_enabled: settings.prompt_optimization_enabled,
            prompt_optimizer_model: settings.prompt_optimizer_model.clone(),
            anthropic_api_key: None,
            replacements: settings.replacements.clone(),
        }
    }
}

#[derive(Clone)]
struct SecretStore {
    service_name: String,
}

impl SecretStore {
    fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    fn entry(&self, account: &str) -> Result<Entry, String> {
        Entry::new(&self.service_name, account)
            .map_err(|error| format!("Failed to access secure storage entry: {error}"))
    }

    fn get_secret(&self, account: &str) -> Result<Option<String>, String> {
        let entry = self.entry(account)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(format!("Failed to read secure setting: {error}")),
        }
    }

    fn write_secret(&self, account: &str, value: &str) -> Result<(), String> {
        let entry = self.entry(account)?;
        if value.trim().is_empty() {
            match entry.delete_credential() {
                Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
                Err(error) => Err(format!("Failed to delete secure setting: {error}")),
            }
        } else {
            entry
                .set_password(value)
                .map_err(|error| format!("Failed to write secure setting: {error}"))
        }
    }
}

pub struct SettingsState {
    pub settings: Mutex<AppSettings>,
    pub path: PathBuf,
    secret_store: SecretStore,
}

impl SettingsState {
    pub fn load(app_dir: PathBuf) -> Self {
        Self::load_with_service_name(app_dir, SETTINGS_SERVICE_NAME)
    }

    fn load_with_service_name(app_dir: PathBuf, service_name: impl Into<String>) -> Self {
        let path = app_dir.join("settings.json");
        let disk_settings = load_disk_settings(&app_dir, &path);
        let secret_store = SecretStore::new(service_name);

        let mut settings = AppSettings {
            transcription_provider: disk_settings.transcription_provider.clone(),
            model: disk_settings.model.clone(),
            language: normalize_language_preference(&disk_settings.language),
            auto_paste: disk_settings.auto_paste,
            preserve_clipboard: disk_settings.preserve_clipboard,
            hotkey: disk_settings.hotkey.clone(),
            widget_mode: disk_settings.widget_mode,
            mic_sensitivity: disk_settings.mic_sensitivity,
            prompt_optimization_enabled: disk_settings.prompt_optimization_enabled,
            prompt_optimizer_model: disk_settings.prompt_optimizer_model.clone(),
            replacements: disk_settings.replacements.clone(),
            ..AppSettings::default()
        };

        let mut needs_resave = settings.language != disk_settings.language
            || disk_settings.api_key.is_some()
            || disk_settings.groq_api_key.is_some()
            || disk_settings.anthropic_api_key.is_some();

        let secret_accounts: [(&str, &mut String, Option<String>); 3] = [
            (OPENAI_API_KEY_ACCOUNT, &mut settings.api_key, disk_settings.api_key.clone()),
            (GROQ_API_KEY_ACCOUNT, &mut settings.groq_api_key, disk_settings.groq_api_key.clone()),
            (ANTHROPIC_API_KEY_ACCOUNT, &mut settings.anthropic_api_key, disk_settings.anthropic_api_key.clone()),
        ];

        for (account, field, disk_fallback) in secret_accounts {
            match secret_store.get_secret(account) {
                Ok(Some(secret)) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Keyring {account}: loaded ({} chars)", secret.len());
                    *field = secret;
                }
                Ok(None) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Keyring {account}: empty");
                    if let Some(secret) = disk_fallback {
                        *field = secret;
                        needs_resave = true;
                    }
                }
                Err(_error) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Keyring {account}: error — {_error}");
                    if let Some(secret) = disk_fallback {
                        *field = secret;
                    }
                }
            }
        }

        let state = Self {
            settings: Mutex::new(settings),
            path,
            secret_store,
        };

        if needs_resave {
            match state.settings.lock() {
                Ok(guard) => {
                    let snapshot = guard.clone();
                    drop(guard);
                    if let Err(_error) = state.persist(&snapshot, None) {
                        #[cfg(debug_assertions)]
                        eprintln!("[FamVoice] Failed to migrate settings into secure storage: {_error}");
                    }
                }
                Err(_error) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Failed to acquire settings lock for migration: {_error}");
                }
            }
        }

        state
    }

    pub fn save_request(&self, request: SaveSettingsRequest) -> Result<AppSettings, String> {
        #[cfg(debug_assertions)]
        eprintln!(
            "[FamVoice] save_request: provider={}, openai={}, groq={}, anthropic={}",
            request.transcription_provider,
            request.api_key.as_deref().map_or("(keep)", |k| if k.is_empty() { "(empty)" } else { "(new)" }),
            request.groq_api_key.as_deref().map_or("(keep)", |k| if k.is_empty() { "(empty)" } else { "(new)" }),
            request.anthropic_api_key.as_deref().map_or("(keep)", |k| if k.is_empty() { "(empty)" } else { "(new)" }),
        );
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| "Failed to lock settings".to_string())?;
        let previous = settings.clone();
        let next = request.merge_with_existing(&previous);

        #[cfg(debug_assertions)]
        eprintln!(
            "[FamVoice] after merge: openai={} chars, groq={} chars, anthropic={} chars",
            next.api_key.len(), next.groq_api_key.len(), next.anthropic_api_key.len()
        );

        if let Err(errors) = validate_settings(&next) {
            return Err(format!("Invalid settings: {}", errors.join(", ")));
        }

        self.persist(&next, Some(&previous))?;
        *settings = next.clone();
        Ok(next)
    }

    fn persist(
        &self,
        settings: &AppSettings,
        previous: Option<&AppSettings>,
    ) -> Result<(), String> {
        if let Err(error) = self.write_secrets(settings) {
            return Err(error);
        }

        if let Err(error) = self.write_disk_settings(settings) {
            if let Some(previous_settings) = previous {
                let _ = self.write_secrets(previous_settings);
            }
            return Err(error);
        }

        Ok(())
    }

    fn write_secrets(&self, settings: &AppSettings) -> Result<(), String> {
        self.secret_store
            .write_secret(OPENAI_API_KEY_ACCOUNT, &settings.api_key)?;
        self.secret_store
            .write_secret(GROQ_API_KEY_ACCOUNT, &settings.groq_api_key)?;
        self.secret_store
            .write_secret(ANTHROPIC_API_KEY_ACCOUNT, &settings.anthropic_api_key)
    }

    fn write_disk_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&DiskSettings::from(settings))
            .map_err(|_| "Failed to serialize settings".to_string())?;
        fs::write(&self.path, data).map_err(|error| format!("Failed to save settings: {error}"))
    }
}

fn load_disk_settings(app_dir: &PathBuf, path: &PathBuf) -> DiskSettings {
    if !path.exists() {
        return DiskSettings::default();
    }

    match fs::read_to_string(path) {
        Ok(data) => match serde_json::from_str::<DiskSettings>(&data) {
            Ok(settings) => settings,
            Err(_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[FamVoice] Failed to parse settings.json: {}, creating backup",
                    _error
                );
                let backup_path = app_dir.join("settings.json.corrupt");
                let _ = fs::copy(path, &backup_path);
                DiskSettings::default()
            }
        },
        Err(_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[FamVoice] Failed to read settings.json: {}, creating backup",
                _error
            );
            let backup_path = app_dir.join("settings.json.corrupt");
            let _ = fs::copy(path, &backup_path);
            DiskSettings::default()
        }
    }
}

pub fn validate_settings(settings: &AppSettings) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if !SUPPORTED_PROVIDERS.contains(&settings.transcription_provider.as_str()) {
        errors.push(format!(
            "Unsupported provider: {}. Use one of: {}",
            settings.transcription_provider,
            SUPPORTED_PROVIDERS.join(", ")
        ));
    }

    if settings.api_key.len() > MAX_API_KEY_LEN {
        errors.push("OpenAI API key is too long".to_string());
    }

    if settings.groq_api_key.len() > MAX_API_KEY_LEN {
        errors.push("Groq API key is too long".to_string());
    }

    if settings.anthropic_api_key.len() > MAX_ANTHROPIC_API_KEY_LEN {
        errors.push(format!(
            "Anthropic API key is too long (max {} chars)",
            MAX_ANTHROPIC_API_KEY_LEN
        ));
    }

    let valid_models = models_for_provider(&settings.transcription_provider);
    if !valid_models.contains(&settings.model.as_str()) {
        errors.push(format!(
            "Unsupported model for {}: {}. Use one of: {}",
            settings.transcription_provider,
            settings.model,
            valid_models.join(", ")
        ));
    }

    if !crate::prompt_optimizer::SUPPORTED_MODELS
        .contains(&settings.prompt_optimizer_model.as_str())
    {
        errors.push(format!(
            "Unsupported prompt optimizer model: {}. Use one of: {}",
            settings.prompt_optimizer_model,
            crate::prompt_optimizer::SUPPORTED_MODELS.join(", ")
        ));
    }

    if !SUPPORTED_LANGUAGE_PREFERENCES.contains(&settings.language.as_str()) {
        errors.push(format!(
            "Invalid language: {}. Use one of: {}",
            settings.language,
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

    for (index, replacement) in settings.replacements.iter().enumerate() {
        if replacement.target.trim().is_empty() {
            errors.push(format!("Replacement {} target cannot be empty", index + 1));
        }
        if replacement.target.len() > 100 {
            errors.push(format!(
                "Replacement {} target is too long (max 100 chars)",
                index + 1
            ));
        }
        if replacement.replacement.len() > 100 {
            errors.push(format!(
                "Replacement {} replacement is too long (max 100 chars)",
                index + 1
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

    fn sample_save_request() -> SaveSettingsRequest {
        SaveSettingsRequest {
            transcription_provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            groq_api_key: None,
            model: "gpt-4o-mini-transcribe".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: false,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            widget_mode: false,
            mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
            prompt_optimization_enabled: false,
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: None,
            replacements: vec![],
        }
    }

    fn sample_settings() -> AppSettings {
        AppSettings {
            transcription_provider: "openai".to_string(),
            api_key: "sk-test".to_string(),
            groq_api_key: String::new(),
            model: "gpt-4o-mini-transcribe".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: false,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            widget_mode: false,
            mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
            prompt_optimization_enabled: false,
            prompt_optimizer_model: "claude-haiku-4-5".to_string(),
            anthropic_api_key: String::new(),
            replacements: vec![],
        }
    }

    fn test_state(dir: &tempfile::TempDir) -> SettingsState {
        let service_name = dir.path().to_string_lossy().replace(['\\', '/', ':'], "_");
        SettingsState::load_with_service_name(
            dir.path().to_path_buf(),
            format!("{SETTINGS_SERVICE_NAME}.test.{service_name}"),
        )
    }

    #[test]
    fn test_to_frontend_masks_secrets() {
        let settings = AppSettings {
            api_key: "sk-test-openai".to_string(),
            groq_api_key: "gsk-test-groq".to_string(),
            anthropic_api_key: "sk-ant-test".to_string(),
            ..sample_settings()
        };

        let frontend = settings.to_frontend();

        assert!(frontend.api_key_present);
        assert_eq!(frontend.api_key_masked.as_deref(), Some("sk-...enai"));
        assert!(frontend.groq_api_key_present);
        assert_eq!(frontend.groq_api_key_masked.as_deref(), Some("gsk...groq"));
        assert!(frontend.anthropic_api_key_present);
        assert_eq!(
            frontend.anthropic_api_key_masked.as_deref(),
            Some("sk-...test")
        );
    }

    #[test]
    fn test_save_request_keeps_existing_secret_when_field_is_none() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        {
            let mut settings = state.settings.lock().expect("Failed to acquire settings lock");
            settings.api_key = "sk-existing".to_string();
            settings.groq_api_key = "gsk-existing".to_string();
            settings.anthropic_api_key = "sk-ant-existing".to_string();
        }

        let saved = state
            .save_request(SaveSettingsRequest {
                api_key: None,
                groq_api_key: None,
                anthropic_api_key: None,
                widget_mode: true,
                prompt_optimization_enabled: true,
                prompt_optimizer_model: "claude-sonnet-4-6".to_string(),
                ..sample_save_request()
            })
            .unwrap();

        assert_eq!(saved.api_key, "sk-existing");
        assert_eq!(saved.groq_api_key, "gsk-existing");
        assert_eq!(saved.anthropic_api_key, "sk-ant-existing");
        assert!(saved.widget_mode);
        assert!(saved.prompt_optimization_enabled);
    }

    #[test]
    fn test_save_request_keeps_existing_secret_when_field_is_blank() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        {
            let mut settings = state.settings.lock().expect("Failed to acquire settings lock");
            settings.api_key = "sk-existing".to_string();
            settings.groq_api_key = "gsk-existing".to_string();
            settings.anthropic_api_key = "sk-ant-existing".to_string();
        }

        let saved = state
            .save_request(SaveSettingsRequest {
                api_key: Some("   ".to_string()),
                groq_api_key: Some("\t".to_string()),
                anthropic_api_key: Some("\n".to_string()),
                ..sample_save_request()
            })
            .unwrap();

        assert_eq!(saved.api_key, "sk-existing");
        assert_eq!(saved.groq_api_key, "gsk-existing");
        assert_eq!(saved.anthropic_api_key, "sk-ant-existing");
    }

    #[test]
    fn test_save_request_persists_sanitized_disk_file() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        state
            .save_request(SaveSettingsRequest {
                groq_api_key: Some("gsk-secret".to_string()),
                anthropic_api_key: Some("sk-ant-secret".to_string()),
                replacements: vec![Replacement {
                    target: "hello".to_string(),
                    replacement: "world".to_string(),
                }],
                ..sample_save_request()
            })
            .unwrap();

        let settings_json = fs::read_to_string(dir.path().join("settings.json")).unwrap();

        assert!(!settings_json.contains("sk-test"));
        assert!(!settings_json.contains("gsk-secret"));
        assert!(!settings_json.contains("sk-ant-secret"));
        assert!(settings_json.contains("\"model\""));
    }

    #[test]
    fn test_load_migrates_legacy_plaintext_secrets() {
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
  "prompt_optimization_enabled": true,
  "prompt_optimizer_model": "claude-sonnet-4-6",
  "anthropic_api_key": "sk-ant-old",
  "groq_api_key": "gsk-old",
  "replacements": []
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state.settings.lock().expect("Failed to acquire settings lock").clone();
        let migrated_json = fs::read_to_string(path).unwrap();

        assert_eq!(settings.api_key, "sk-test");
        assert_eq!(settings.groq_api_key, "gsk-old");
        assert_eq!(settings.anthropic_api_key, "sk-ant-old");
        assert_eq!(settings.language, "pt");
        assert!(!migrated_json.contains("sk-test"));
        assert!(!migrated_json.contains("gsk-old"));
        assert!(!migrated_json.contains("sk-ant-old"));
        assert!(!migrated_json.contains("pt-first"));
    }

    #[test]
    fn test_validate_settings_valid() {
        assert!(validate_settings(&sample_settings()).is_ok());
    }

    #[test]
    fn test_validate_settings_rejects_invalid_model() {
        let settings = AppSettings {
            model: "invalid-model".to_string(),
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| error.contains("Unsupported model")));
    }

    #[test]
    fn test_validate_settings_rejects_invalid_language() {
        let settings = AppSettings {
            language: "invalid".to_string(),
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| error.contains("Invalid language")));
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
            .any(|error| error.contains("Mic sensitivity")));
    }
}
