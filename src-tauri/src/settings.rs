use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SETTINGS_SERVICE_NAME: &str = "com.famvoice.app";
const OPENAI_API_KEY_ACCOUNT: &str = "openai_api_key";
const GROQ_API_KEY_ACCOUNT: &str = "groq_api_key";
const OPENAI_API_KEY_CONTEXT: &str = "OpenAI API key";
const GROQ_API_KEY_CONTEXT: &str = "Groq API key";
const MAX_API_KEY_LEN: usize = 200;
const MAX_HOTKEY_LEN: usize = 100;
const MAX_INPUT_DEVICE_ID_LEN: usize = 512;
pub const SUPPORTED_PROVIDERS: [&str; 2] = ["openai", "groq"];
pub const OPENAI_MODELS: [&str; 2] = ["gpt-transcribe", "whisper-1"];
pub const GROQ_MODELS: [&str; 2] = ["whisper-large-v3-turbo", "whisper-large-v3"];
const DEFAULT_OPENAI_MODEL: &str = "gpt-transcribe";
const DEFAULT_GROQ_MODEL: &str = "whisper-large-v3-turbo";
const CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION: u32 = 1;
const TRANSCRIPTION_MODEL_MIGRATION_NOTICE: &str = "FamVoice updated your legacy OpenAI transcription model to gpt-transcribe, the recommended model for completed dictation. You can still choose whisper-1 for timestamps, subtitles, or translation.";
pub const SUPPORTED_LANGUAGE_PREFERENCES: [&str; 17] = [
    "auto", "ar", "de", "en", "es", "fr", "hi", "it", "ja", "ko", "nl", "pl", "pt", "ru", "tr",
    "uk", "zh",
];
pub const MIN_MIC_SENSITIVITY: u8 = 0;
pub const MAX_MIC_SENSITIVITY: u8 = 100;
pub const DEFAULT_MIC_SENSITIVITY: u8 = 60;

#[derive(Clone, Serialize, Deserialize)]
pub struct Replacement {
    pub target: String,
    pub replacement: String,
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_model() -> String {
    DEFAULT_OPENAI_MODEL.to_string()
}

fn models_for_provider(provider: &str) -> &'static [&'static str] {
    match provider {
        "groq" => &GROQ_MODELS,
        _ => &OPENAI_MODELS,
    }
}

fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "groq" => DEFAULT_GROQ_MODEL,
        _ => DEFAULT_OPENAI_MODEL,
    }
}

fn normalize_transcription_model(provider: &str, model: &str) -> String {
    if models_for_provider(provider).contains(&model) {
        model.to_string()
    } else {
        default_model_for_provider(provider).to_string()
    }
}

fn migrate_transcription_model(settings: &DiskSettings) -> (String, bool) {
    if settings.transcription_model_settings_version < CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION
        && settings.transcription_provider == "openai"
    {
        return (DEFAULT_OPENAI_MODEL.to_string(), true);
    }

    (
        normalize_transcription_model(&settings.transcription_provider, &settings.model),
        false,
    )
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

fn default_repaste_hotkey() -> String {
    String::new()
}

fn default_widget_mode() -> bool {
    false
}

fn default_input_device_id() -> String {
    String::new()
}

fn default_mic_sensitivity() -> u8 {
    DEFAULT_MIC_SENSITIVITY
}

fn default_noise_suppression_enabled() -> bool {
    false
}

fn default_prompt_optimization_enabled() -> bool {
    false
}

fn default_prompt_optimizer_model() -> String {
    crate::prompt_optimizer::SUPPORTED_MODELS[0].to_string()
}

fn normalize_prompt_optimizer_model(model: &str) -> String {
    if crate::prompt_optimizer::SUPPORTED_MODELS.contains(&model) {
        model.to_string()
    } else {
        default_prompt_optimizer_model()
    }
}

fn normalize_language_preference(language: &str) -> String {
    let resolved = match language {
        "pt-first" => "pt",
        "en-first" => "en",
        other => other,
    };
    if SUPPORTED_LANGUAGE_PREFERENCES.contains(&resolved) {
        resolved.to_string()
    } else {
        "auto".to_string()
    }
}

fn default_replacements() -> Vec<Replacement> {
    Vec::new()
}

pub(crate) fn normalize_input_device_id(input_device_id: &str) -> String {
    let trimmed = input_device_id.trim();
    if trimmed.is_empty() {
        default_input_device_id()
    } else {
        trimmed.to_string()
    }
}

fn normalize_repaste_hotkey(hotkey: &str) -> String {
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        default_repaste_hotkey()
    } else {
        trimmed.to_string()
    }
}

fn mask_secret(secret: &str) -> Option<String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().count() <= 7 {
        return Some("***".to_string());
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
    pub repaste_hotkey: String,
    pub widget_mode: bool,
    pub input_device_id: String,
    pub mic_sensitivity: u8,
    pub noise_suppression_enabled: bool,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
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
            repaste_hotkey: default_repaste_hotkey(),
            widget_mode: default_widget_mode(),
            input_device_id: default_input_device_id(),
            mic_sensitivity: default_mic_sensitivity(),
            noise_suppression_enabled: default_noise_suppression_enabled(),
            prompt_optimization_enabled: default_prompt_optimization_enabled(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
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
            repaste_hotkey: self.repaste_hotkey.clone(),
            widget_mode: self.widget_mode,
            input_device_id: self.input_device_id.clone(),
            mic_sensitivity: self.mic_sensitivity,
            noise_suppression_enabled: self.noise_suppression_enabled,
            prompt_optimization_enabled: self.prompt_optimization_enabled,
            prompt_optimizer_model: self.prompt_optimizer_model.clone(),
            replacements: self.replacements.clone(),
            credential_storage: CredentialStorageState::secure(),
            transcription_model_notice: None,
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
    pub repaste_hotkey: String,
    pub widget_mode: bool,
    pub input_device_id: String,
    pub mic_sensitivity: u8,
    pub noise_suppression_enabled: bool,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
    pub replacements: Vec<Replacement>,
    pub credential_storage: CredentialStorageState,
    pub transcription_model_notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CredentialStorageState {
    pub mode: String,
    pub message: Option<String>,
}

impl CredentialStorageState {
    fn secure() -> Self {
        Self {
            mode: "secure_store".to_string(),
            message: None,
        }
    }

    fn encrypted_fallback() -> Self {
        Self {
            mode: "encrypted_disk_fallback".to_string(),
            message: Some(
                "Windows Credential Manager is unavailable. Existing keys were recovered from the encrypted local copy; reopen Settings and retry before changing keys."
                    .to_string(),
            ),
        }
    }
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
    pub repaste_hotkey: String,
    pub widget_mode: bool,
    pub input_device_id: String,
    pub mic_sensitivity: u8,
    pub noise_suppression_enabled: bool,
    pub prompt_optimization_enabled: bool,
    pub prompt_optimizer_model: String,
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
            repaste_hotkey: normalize_repaste_hotkey(&self.repaste_hotkey),
            widget_mode: self.widget_mode,
            input_device_id: normalize_input_device_id(&self.input_device_id),
            mic_sensitivity: self.mic_sensitivity,
            noise_suppression_enabled: self.noise_suppression_enabled,
            prompt_optimization_enabled: self.prompt_optimization_enabled,
            prompt_optimizer_model: self.prompt_optimizer_model,
            replacements: self.replacements,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct DiskSettings {
    #[serde(default)]
    transcription_model_settings_version: u32,
    #[serde(default = "default_provider")]
    transcription_provider: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    groq_api_key: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_encrypted: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    groq_api_key_encrypted: Option<String>,
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
    #[serde(default = "default_repaste_hotkey")]
    repaste_hotkey: String,
    #[serde(default = "default_widget_mode")]
    widget_mode: bool,
    #[serde(default = "default_input_device_id")]
    input_device_id: String,
    #[serde(default = "default_mic_sensitivity")]
    mic_sensitivity: u8,
    #[serde(default = "default_noise_suppression_enabled")]
    noise_suppression_enabled: bool,
    #[serde(default = "default_prompt_optimization_enabled")]
    prompt_optimization_enabled: bool,
    #[serde(default = "default_prompt_optimizer_model")]
    prompt_optimizer_model: String,
    #[serde(default, alias = "anthropic_api_key")]
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_anthropic_api_key: Option<String>,
    #[serde(default = "default_replacements")]
    replacements: Vec<Replacement>,
}

impl Default for DiskSettings {
    fn default() -> Self {
        Self {
            transcription_model_settings_version: CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION,
            transcription_provider: default_provider(),
            api_key: None,
            groq_api_key: None,
            api_key_encrypted: None,
            groq_api_key_encrypted: None,
            model: default_model(),
            language: default_language(),
            auto_paste: default_auto_paste(),
            preserve_clipboard: default_preserve_clipboard(),
            hotkey: default_hotkey(),
            repaste_hotkey: default_repaste_hotkey(),
            widget_mode: default_widget_mode(),
            input_device_id: default_input_device_id(),
            mic_sensitivity: default_mic_sensitivity(),
            noise_suppression_enabled: default_noise_suppression_enabled(),
            prompt_optimization_enabled: default_prompt_optimization_enabled(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            legacy_anthropic_api_key: None,
            replacements: default_replacements(),
        }
    }
}

impl DiskSettings {
    fn from_settings(settings: &AppSettings) -> Result<Self, String> {
        Ok(Self {
            transcription_model_settings_version: CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION,
            transcription_provider: settings.transcription_provider.clone(),
            api_key: None,
            groq_api_key: None,
            api_key_encrypted: encrypt_optional_secret(&settings.api_key, OPENAI_API_KEY_CONTEXT)?,
            groq_api_key_encrypted: encrypt_optional_secret(
                &settings.groq_api_key,
                GROQ_API_KEY_CONTEXT,
            )?,
            model: settings.model.clone(),
            language: settings.language.clone(),
            auto_paste: settings.auto_paste,
            preserve_clipboard: settings.preserve_clipboard,
            hotkey: settings.hotkey.clone(),
            repaste_hotkey: settings.repaste_hotkey.clone(),
            widget_mode: settings.widget_mode,
            input_device_id: settings.input_device_id.clone(),
            mic_sensitivity: settings.mic_sensitivity,
            noise_suppression_enabled: settings.noise_suppression_enabled,
            prompt_optimization_enabled: settings.prompt_optimization_enabled,
            prompt_optimizer_model: settings.prompt_optimizer_model.clone(),
            legacy_anthropic_api_key: None,
            replacements: settings.replacements.clone(),
        })
    }
}

#[derive(Clone)]
struct SecretStore {
    service_name: String,
}

type SecretAccount<'a> = (
    &'static str,
    &'static str,
    &'a mut String,
    Option<String>,
    Option<String>,
);

trait CredentialStore: Send + Sync {
    fn get_secret(&self, account: &str) -> Result<Option<String>, String>;
    fn write_secret(&self, account: &str, value: &str) -> Result<(), String>;
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
}

impl CredentialStore for SecretStore {
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
    storage: crate::persistence::AtomicFile,
    secret_store: Arc<dyn CredentialStore>,
    credential_storage: Mutex<CredentialStorageState>,
    transcription_model_notice: Option<String>,
}

impl SettingsState {
    pub fn load(app_dir: PathBuf) -> Self {
        Self::load_with_service_name(app_dir, SETTINGS_SERVICE_NAME)
    }

    fn load_with_service_name(app_dir: PathBuf, service_name: impl Into<String>) -> Self {
        Self::load_with_store(app_dir, Arc::new(SecretStore::new(service_name)))
    }

    fn load_with_store(app_dir: PathBuf, secret_store: Arc<dyn CredentialStore>) -> Self {
        let path = app_dir.join("settings.json");
        let storage = crate::persistence::AtomicFile::new(path);
        let (disk_settings, recovered_from_backup) = load_disk_settings(&storage);
        let (transcription_model, migrated_transcription_model) =
            migrate_transcription_model(&disk_settings);

        let mut settings = AppSettings {
            transcription_provider: disk_settings.transcription_provider.clone(),
            model: transcription_model,
            language: normalize_language_preference(&disk_settings.language),
            auto_paste: disk_settings.auto_paste,
            preserve_clipboard: disk_settings.preserve_clipboard,
            hotkey: disk_settings.hotkey.clone(),
            repaste_hotkey: normalize_repaste_hotkey(&disk_settings.repaste_hotkey),
            widget_mode: disk_settings.widget_mode,
            input_device_id: normalize_input_device_id(&disk_settings.input_device_id),
            mic_sensitivity: disk_settings.mic_sensitivity,
            noise_suppression_enabled: disk_settings.noise_suppression_enabled,
            prompt_optimization_enabled: disk_settings.prompt_optimization_enabled,
            prompt_optimizer_model: normalize_prompt_optimizer_model(
                &disk_settings.prompt_optimizer_model,
            ),
            replacements: disk_settings.replacements.clone(),
            ..AppSettings::default()
        };

        let mut needs_resave = recovered_from_backup
            || disk_settings.transcription_model_settings_version
                < CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION
            || settings.language != disk_settings.language
            || settings.model != disk_settings.model
            || settings.repaste_hotkey != disk_settings.repaste_hotkey
            || settings.input_device_id != disk_settings.input_device_id
            || settings.prompt_optimizer_model != disk_settings.prompt_optimizer_model
            || disk_settings.api_key.is_some()
            || disk_settings.groq_api_key.is_some()
            || disk_settings.legacy_anthropic_api_key.is_some();
        let mut accounts_to_seed = Vec::new();
        let mut keyring_unavailable = false;

        let secret_accounts: [SecretAccount<'_>; 2] = [
            (
                OPENAI_API_KEY_ACCOUNT,
                OPENAI_API_KEY_CONTEXT,
                &mut settings.api_key,
                disk_settings.api_key.clone(),
                disk_settings.api_key_encrypted.clone(),
            ),
            (
                GROQ_API_KEY_ACCOUNT,
                GROQ_API_KEY_CONTEXT,
                &mut settings.groq_api_key,
                disk_settings.groq_api_key.clone(),
                disk_settings.groq_api_key_encrypted.clone(),
            ),
        ];

        // Authority rule: a readable keyring value wins and is mirrored to the
        // DPAPI fallback. The encrypted disk value is authoritative only when
        // the keyring entry is absent or the keyring cannot be read.
        for (account, context, field, plaintext_fallback, encrypted_fallback) in secret_accounts {
            match secret_store.get_secret(account) {
                Ok(Some(secret)) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[FamVoice] Keyring {account}: loaded ({} chars)",
                        secret.len()
                    );
                    let fallback_matches = recover_disk_secret(
                        encrypted_fallback.as_deref(),
                        plaintext_fallback.as_deref(),
                        context,
                    )
                    .ok()
                    .flatten()
                    .as_deref()
                        == Some(secret.as_str());
                    if !fallback_matches {
                        needs_resave = true;
                    }
                    *field = secret;
                }
                Ok(None) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Keyring {account}: empty");
                    let recovered_secret = match recover_disk_secret(
                        encrypted_fallback.as_deref(),
                        plaintext_fallback.as_deref(),
                        context,
                    ) {
                        Ok(secret) => secret,
                        Err(error) => {
                            #[cfg(debug_assertions)]
                            eprintln!("[FamVoice] Failed to recover {account} from disk: {error}");
                            None
                        }
                    };
                    if let Some(secret) = recovered_secret {
                        *field = secret;
                        needs_resave = true;
                        accounts_to_seed.push(account);
                    }
                }
                Err(error) => {
                    keyring_unavailable = true;
                    #[cfg(debug_assertions)]
                    eprintln!("[FamVoice] Keyring {account}: error — {error}");
                    let recovered_secret = match recover_disk_secret(
                        encrypted_fallback.as_deref(),
                        plaintext_fallback.as_deref(),
                        context,
                    ) {
                        Ok(secret) => secret,
                        Err(recovery_error) => {
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "[FamVoice] Failed to recover {account} after keyring error: {recovery_error}"
                            );
                            None
                        }
                    };
                    if let Some(secret) = recovered_secret {
                        *field = secret;
                        needs_resave = true;
                        accounts_to_seed.push(account);
                    }
                }
            }
        }

        let state = Self {
            settings: Mutex::new(settings),
            storage,
            secret_store,
            credential_storage: Mutex::new(if keyring_unavailable {
                CredentialStorageState::encrypted_fallback()
            } else {
                CredentialStorageState::secure()
            }),
            transcription_model_notice: migrated_transcription_model
                .then(|| TRANSCRIPTION_MODEL_MIGRATION_NOTICE.to_string()),
        };

        if needs_resave {
            match state.settings.lock() {
                Ok(guard) => {
                    let snapshot = guard.clone();
                    drop(guard);
                    if let Err(_error) = state.write_migrated_disk_settings(&snapshot) {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "[FamVoice] Failed to migrate settings to atomic encrypted storage: {_error}"
                        );
                    } else if !accounts_to_seed.is_empty() {
                        match state.write_selected_secrets(&snapshot, &accounts_to_seed) {
                            Ok(()) => {
                                state.set_credential_storage(CredentialStorageState::secure())
                            }
                            Err(_error) => {
                                state.set_credential_storage(
                                    CredentialStorageState::encrypted_fallback(),
                                );
                                #[cfg(debug_assertions)]
                                eprintln!(
                                    "[FamVoice] Failed to seed recovered credentials into the keyring: {_error}"
                                );
                            }
                        }
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

    pub fn apply_credential_state(&self, frontend: &mut FrontendSettings) {
        frontend.credential_storage = self
            .credential_storage
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        frontend.transcription_model_notice = self.transcription_model_notice.clone();
    }

    fn set_credential_storage(&self, value: CredentialStorageState) {
        *self
            .credential_storage
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = value;
    }

    pub fn save_request(&self, request: SaveSettingsRequest) -> Result<AppSettings, String> {
        #[cfg(debug_assertions)]
        eprintln!(
            "[FamVoice] save_request: provider={}, openai={}, groq={}",
            request.transcription_provider,
            request
                .api_key
                .as_deref()
                .map_or("(keep)", |k| if k.is_empty() { "(empty)" } else { "(new)" }),
            request
                .groq_api_key
                .as_deref()
                .map_or("(keep)", |k| if k.is_empty() { "(empty)" } else { "(new)" }),
        );
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| "Failed to lock settings".to_string())?;
        let previous = settings.clone();
        let next = request.merge_with_existing(&previous);

        #[cfg(debug_assertions)]
        eprintln!(
            "[FamVoice] after merge: openai={} chars, groq={} chars",
            next.api_key.len(),
            next.groq_api_key.len()
        );

        if let Err(errors) = validate_settings(&next) {
            return Err(format!("Invalid settings: {}", errors.join(", ")));
        }

        self.persist(&next, &previous)?;
        *settings = next.clone();
        Ok(next)
    }

    fn persist(&self, settings: &AppSettings, previous: &AppSettings) -> Result<(), String> {
        let changes = secret_changes(settings, previous);
        let applied = match self.apply_secret_changes(&changes) {
            Ok(applied) => applied,
            Err((error, rollback_complete)) => {
                #[cfg(debug_assertions)]
                eprintln!("[FamVoice] Credential save failed: {error}");
                if !rollback_complete {
                    self.set_credential_storage(CredentialStorageState::encrypted_fallback());
                }
                return Err(sanitized_credential_save_error(rollback_complete));
            }
        };

        if let Err(error) = self.write_disk_settings(settings) {
            let rollback_complete = self.rollback_secret_changes(&applied);
            if !rollback_complete {
                self.set_credential_storage(CredentialStorageState::encrypted_fallback());
            }
            return Err(if rollback_complete {
                error
            } else {
                "Settings were not committed and credential recovery needs attention. Reopen Settings before retrying."
                    .to_string()
            });
        }

        self.set_credential_storage(CredentialStorageState::secure());
        Ok(())
    }

    fn apply_secret_changes<'a>(
        &self,
        changes: &[SecretChange<'a>],
    ) -> Result<Vec<SecretChange<'a>>, (String, bool)> {
        let mut applied = Vec::new();
        for change in changes {
            if let Err(error) = self.secret_store.write_secret(change.account, change.next) {
                let rollback_complete = self.rollback_secret_changes(&applied);
                return Err((error, rollback_complete));
            }
            applied.push(*change);
        }
        Ok(applied)
    }

    fn rollback_secret_changes(&self, changes: &[SecretChange<'_>]) -> bool {
        let mut complete = true;
        for change in changes.iter().rev() {
            if self
                .secret_store
                .write_secret(change.account, change.previous)
                .is_err()
            {
                complete = false;
            }
        }
        complete
    }

    fn write_selected_secrets(
        &self,
        settings: &AppSettings,
        accounts: &[&str],
    ) -> Result<(), String> {
        for account in accounts {
            let value = match *account {
                OPENAI_API_KEY_ACCOUNT => &settings.api_key,
                GROQ_API_KEY_ACCOUNT => &settings.groq_api_key,
                _ => continue,
            };
            self.secret_store.write_secret(account, value)?;
        }
        Ok(())
    }

    fn write_disk_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&DiskSettings::from_settings(settings)?)
            .map_err(|_| "Failed to serialize settings".to_string())?;
        let revision = self.storage.reserve_revision();
        self.storage.write(revision, data.as_bytes()).map(|_| ())
    }

    fn write_migrated_disk_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let data = serde_json::to_string_pretty(&DiskSettings::from_settings(settings)?)
            .map_err(|_| "Failed to serialize settings".to_string())?;
        // Legacy files can contain plaintext secrets. Replace atomically without
        // copying that plaintext into the recovery file.
        self.storage.restore_known_good(data.as_bytes())
    }
}

#[derive(Clone, Copy)]
struct SecretChange<'a> {
    account: &'static str,
    previous: &'a str,
    next: &'a str,
}

fn secret_changes<'a>(
    settings: &'a AppSettings,
    previous: &'a AppSettings,
) -> Vec<SecretChange<'a>> {
    [
        SecretChange {
            account: OPENAI_API_KEY_ACCOUNT,
            previous: &previous.api_key,
            next: &settings.api_key,
        },
        SecretChange {
            account: GROQ_API_KEY_ACCOUNT,
            previous: &previous.groq_api_key,
            next: &settings.groq_api_key,
        },
    ]
    .into_iter()
    .filter(|change| change.previous != change.next)
    .collect()
}

fn sanitized_credential_save_error(rollback_complete: bool) -> String {
    if rollback_complete {
        "Windows Credential Manager is unavailable. No settings were changed. Reopen Settings and try again."
            .to_string()
    } else {
        "Credential storage failed during recovery. Settings were not committed; reopen Settings before retrying."
            .to_string()
    }
}

fn recover_disk_secret(
    encrypted_secret: Option<&str>,
    plaintext_secret: Option<&str>,
    context: &str,
) -> Result<Option<String>, String> {
    if let Some(secret) = encrypted_secret.filter(|secret| !secret.trim().is_empty()) {
        return decrypt_disk_secret(secret, context).map(Some);
    }

    Ok(plaintext_secret
        .map(str::trim)
        .filter(|secret| !secret.is_empty())
        .map(str::to_string))
}

fn encrypt_optional_secret(secret: &str, context: &str) -> Result<Option<String>, String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    #[cfg(windows)]
    {
        crate::dpapi::protect_string(trimmed, context).map(Some)
    }

    #[cfg(not(windows))]
    {
        let _ = context;
        Ok(None)
    }
}

fn decrypt_disk_secret(secret: &str, context: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        crate::dpapi::unprotect_string(secret, context)
    }

    #[cfg(not(windows))]
    {
        let _ = context;
        Err("Encrypted disk secrets are only supported on Windows".to_string())
    }
}

fn load_disk_settings(storage: &crate::persistence::AtomicFile) -> (DiskSettings, bool) {
    let path = storage.path();
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(data) => match serde_json::from_str::<DiskSettings>(&data) {
                Ok(settings) => return (settings, false),
                Err(_error) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[FamVoice] Failed to parse settings.json: {}, preserving corrupt file",
                        _error
                    );
                    let _ = crate::persistence::preserve_corrupt_file(path);
                }
            },
            Err(_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[FamVoice] Failed to read settings.json: {}, preserving corrupt file",
                    _error
                );
                let _ = crate::persistence::preserve_corrupt_file(path);
            }
        }
    }

    match fs::read_to_string(storage.backup_path()) {
        Ok(data) => match serde_json::from_str::<DiskSettings>(&data) {
            Ok(settings) => (settings, true),
            Err(_error) => {
                #[cfg(debug_assertions)]
                eprintln!("[FamVoice] Settings recovery copy is invalid: {_error}");
                (DiskSettings::default(), false)
            }
        },
        Err(_) => (DiskSettings::default(), false),
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

    if settings.hotkey.len() > MAX_HOTKEY_LEN {
        errors.push("Hotkey is too long".to_string());
    }

    if settings.repaste_hotkey.len() > MAX_HOTKEY_LEN {
        errors.push("Re-paste hotkey is too long".to_string());
    }

    if !settings.repaste_hotkey.is_empty() && settings.repaste_hotkey.starts_with("Mouse") {
        errors.push("Re-paste hotkey must use a keyboard shortcut".to_string());
    }

    if !settings.repaste_hotkey.is_empty() && settings.repaste_hotkey == settings.hotkey {
        errors.push("Re-paste hotkey must be different from the recording hotkey".to_string());
    }

    if settings.input_device_id.len() > MAX_INPUT_DEVICE_ID_LEN {
        errors.push("Input device id is too long".to_string());
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
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::tempdir;

    #[derive(Default)]
    struct MockCredentialStore {
        secrets: Mutex<HashMap<String, String>>,
        unavailable: AtomicBool,
        fail_write_account: Mutex<Option<String>>,
    }

    impl MockCredentialStore {
        fn with_secrets(values: &[(&str, &str)]) -> Self {
            Self {
                secrets: Mutex::new(
                    values
                        .iter()
                        .map(|(account, value)| (account.to_string(), value.to_string()))
                        .collect(),
                ),
                ..Self::default()
            }
        }

        fn secret(&self, account: &str) -> Option<String> {
            self.secrets.lock().unwrap().get(account).cloned()
        }
    }

    impl CredentialStore for MockCredentialStore {
        fn get_secret(&self, account: &str) -> Result<Option<String>, String> {
            if self.unavailable.load(Ordering::SeqCst) {
                return Err("simulated credential store unavailable".to_string());
            }
            Ok(self.secret(account))
        }

        fn write_secret(&self, account: &str, value: &str) -> Result<(), String> {
            if self.unavailable.load(Ordering::SeqCst)
                || self.fail_write_account.lock().unwrap().as_deref() == Some(account)
            {
                return Err("simulated credential write failure".to_string());
            }

            let mut secrets = self.secrets.lock().unwrap();
            if value.trim().is_empty() {
                secrets.remove(account);
            } else {
                secrets.insert(account.to_string(), value.to_string());
            }
            Ok(())
        }
    }

    fn sample_save_request() -> SaveSettingsRequest {
        SaveSettingsRequest {
            transcription_provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            groq_api_key: None,
            model: "whisper-1".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: false,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            repaste_hotkey: String::new(),
            widget_mode: false,
            input_device_id: String::new(),
            mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
            noise_suppression_enabled: false,
            prompt_optimization_enabled: false,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            replacements: vec![],
        }
    }

    fn sample_settings() -> AppSettings {
        AppSettings {
            transcription_provider: "openai".to_string(),
            api_key: "sk-test".to_string(),
            groq_api_key: String::new(),
            model: "whisper-1".to_string(),
            language: "auto".to_string(),
            auto_paste: true,
            preserve_clipboard: false,
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            repaste_hotkey: String::new(),
            widget_mode: false,
            input_device_id: String::new(),
            mic_sensitivity: DEFAULT_MIC_SENSITIVITY,
            noise_suppression_enabled: false,
            prompt_optimization_enabled: false,
            prompt_optimizer_model: "gpt-5.4-mini".to_string(),
            replacements: vec![],
        }
    }

    fn test_state(dir: &tempfile::TempDir) -> SettingsState {
        SettingsState::load_with_store(
            dir.path().to_path_buf(),
            Arc::new(MockCredentialStore::default()),
        )
    }

    #[test]
    fn test_default_uses_recommended_openai_transcription_model() {
        let settings = AppSettings::default();
        let disk_settings = DiskSettings::default();

        assert_eq!(settings.transcription_provider, "openai");
        assert_eq!(settings.model, "gpt-transcribe");
        assert_eq!(
            disk_settings.transcription_model_settings_version,
            CURRENT_TRANSCRIPTION_MODEL_SETTINGS_VERSION
        );
    }

    #[test]
    fn test_legacy_openai_whisper_migrates_once_with_sanitized_notice() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "transcription_provider": "openai",
  "api_key": "sk-never-show-this",
  "model": "whisper-1",
  "language": "pt"
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state.settings.lock().unwrap().clone();
        let mut frontend = settings.to_frontend();
        state.apply_credential_state(&mut frontend);

        assert_eq!(settings.model, "gpt-transcribe");
        let notice = frontend
            .transcription_model_notice
            .expect("legacy OpenAI migration should be visible");
        assert!(notice.contains("gpt-transcribe"));
        assert!(!notice.contains("sk-never-show-this"));

        let migrated_json = fs::read_to_string(&path).unwrap();
        assert!(migrated_json.contains(r#""transcription_model_settings_version": 1"#));
        assert!(migrated_json.contains(r#""model": "gpt-transcribe""#));
        assert!(!migrated_json.contains("sk-never-show-this"));
        drop(state);

        let reloaded = test_state(&dir);
        let reloaded_settings = reloaded.settings.lock().unwrap().clone();
        let mut reloaded_frontend = reloaded_settings.to_frontend();
        reloaded.apply_credential_state(&mut reloaded_frontend);

        assert_eq!(reloaded_settings.model, "gpt-transcribe");
        assert_eq!(reloaded_frontend.transcription_model_notice, None);
    }

    #[test]
    fn test_current_explicit_openai_whisper_choice_is_preserved() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "transcription_model_settings_version": 1,
  "transcription_provider": "openai",
  "model": "whisper-1",
  "language": "pt"
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state.settings.lock().unwrap().clone();
        let mut frontend = settings.to_frontend();
        state.apply_credential_state(&mut frontend);

        assert_eq!(settings.model, "whisper-1");
        assert_eq!(frontend.transcription_model_notice, None);
        assert!(fs::read_to_string(path)
            .unwrap()
            .contains(r#""model": "whisper-1""#));
    }

    #[test]
    fn test_legacy_groq_model_choices_are_preserved() {
        for model in GROQ_MODELS {
            let dir = tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(
                &path,
                format!(
                    r#"{{
  "transcription_provider": "groq",
  "model": "{model}",
  "language": "pt"
}}"#
                ),
            )
            .unwrap();

            let state = test_state(&dir);
            let settings = state.settings.lock().unwrap().clone();
            let mut frontend = settings.to_frontend();
            state.apply_credential_state(&mut frontend);

            assert_eq!(settings.model, model);
            assert_eq!(frontend.transcription_model_notice, None);
            let migrated_json = fs::read_to_string(path).unwrap();
            assert!(migrated_json.contains(&format!(r#""model": "{model}""#)));
            assert!(migrated_json.contains(r#""transcription_model_settings_version": 1"#));
        }
    }

    #[test]
    fn test_missing_and_unsupported_legacy_openai_models_use_recommended_default() {
        for model_field in [
            String::new(),
            r#", "model": "unsupported-openai-model""#.to_string(),
        ] {
            let dir = tempdir().unwrap();
            fs::write(
                dir.path().join("settings.json"),
                format!(r#"{{"transcription_provider": "openai"{model_field}, "language": "pt"}}"#),
            )
            .unwrap();

            let state = test_state(&dir);
            let settings = state.settings.lock().unwrap().clone();
            let mut frontend = settings.to_frontend();
            state.apply_credential_state(&mut frontend);

            assert_eq!(settings.model, "gpt-transcribe");
            assert!(frontend.transcription_model_notice.is_some());
        }
    }

    #[test]
    fn test_to_frontend_masks_secrets() {
        let settings = AppSettings {
            api_key: "sk-test-openai".to_string(),
            groq_api_key: "gsk-test-groq".to_string(),
            ..sample_settings()
        };

        let frontend = settings.to_frontend();

        assert!(frontend.api_key_present);
        assert_eq!(frontend.api_key_masked.as_deref(), Some("sk-...enai"));
        assert!(frontend.groq_api_key_present);
        assert_eq!(frontend.groq_api_key_masked.as_deref(), Some("gsk...groq"));
    }

    #[test]
    fn test_to_frontend_does_not_reveal_short_secrets() {
        let settings = AppSettings {
            api_key: "secret".to_string(),
            ..sample_settings()
        };

        let frontend = settings.to_frontend();

        assert_eq!(frontend.api_key_masked.as_deref(), Some("***"));
        assert!(!frontend.api_key_masked.unwrap().contains("secret"));
    }

    #[test]
    fn test_save_request_keeps_existing_secret_when_field_is_none() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        {
            let mut settings = state
                .settings
                .lock()
                .expect("Failed to acquire settings lock");
            settings.api_key = "sk-existing".to_string();
            settings.groq_api_key = "gsk-existing".to_string();
        }

        let saved = state
            .save_request(SaveSettingsRequest {
                api_key: None,
                groq_api_key: None,
                widget_mode: true,
                prompt_optimization_enabled: true,
                prompt_optimizer_model: "gpt-5.4-mini".to_string(),
                ..sample_save_request()
            })
            .unwrap();

        assert_eq!(saved.api_key, "sk-existing");
        assert_eq!(saved.groq_api_key, "gsk-existing");
        assert!(saved.widget_mode);
        assert!(saved.prompt_optimization_enabled);
    }

    #[test]
    fn test_save_request_keeps_existing_secret_when_field_is_blank() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        {
            let mut settings = state
                .settings
                .lock()
                .expect("Failed to acquire settings lock");
            settings.api_key = "sk-existing".to_string();
            settings.groq_api_key = "gsk-existing".to_string();
        }

        let saved = state
            .save_request(SaveSettingsRequest {
                api_key: Some("   ".to_string()),
                groq_api_key: Some("\t".to_string()),
                ..sample_save_request()
            })
            .unwrap();

        assert_eq!(saved.api_key, "sk-existing");
        assert_eq!(saved.groq_api_key, "gsk-existing");
    }

    #[test]
    fn test_save_request_persists_sanitized_disk_file() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        state
            .save_request(SaveSettingsRequest {
                groq_api_key: Some("gsk-secret".to_string()),
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
        assert!(settings_json.contains("\"model\""));
    }

    #[test]
    fn test_save_request_persists_preserve_clipboard_disabled_without_inversion() {
        let dir = tempdir().unwrap();
        let state = test_state(&dir);

        let initial = state
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();
        assert!(initial.preserve_clipboard);

        let saved = state
            .save_request(SaveSettingsRequest {
                preserve_clipboard: false,
                ..sample_save_request()
            })
            .unwrap();

        assert!(!saved.preserve_clipboard);

        let settings_json = fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(settings_json.contains("\"preserve_clipboard\": false"));

        let reloaded = test_state(&dir);
        let reloaded_settings = reloaded
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();
        assert!(!reloaded_settings.preserve_clipboard);
    }

    #[test]
    fn test_load_migrates_legacy_plaintext_secrets() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "api_key": "sk-test",
  "model": "gpt-4o-transcribe",
  "language": "pt-first",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "prompt_optimization_enabled": true,
  "prompt_optimizer_model": "gpt-5.4-mini",
  "anthropic_api_key": "sk-ant-old",
  "groq_api_key": "gsk-old",
  "replacements": []
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();
        let migrated_json = fs::read_to_string(path).unwrap();

        assert_eq!(settings.api_key, "sk-test");
        assert_eq!(settings.groq_api_key, "gsk-old");
        assert_eq!(settings.model, "gpt-transcribe");
        assert_eq!(settings.language, "pt");
        assert!(!migrated_json.contains("sk-test"));
        assert!(!migrated_json.contains("gsk-old"));
        assert!(!migrated_json.contains("sk-ant-old"));
        assert!(migrated_json.contains(r#""model": "gpt-transcribe""#));
        assert!(migrated_json.contains(r#""transcription_model_settings_version": 1"#));
        assert!(!migrated_json.contains("gpt-4o-transcribe"));
        assert!(!migrated_json.contains("pt-first"));
        assert!(!dir.path().join("settings.json.bak").exists());
    }

    #[cfg(windows)]
    #[test]
    fn test_keyring_unavailable_recovers_from_encrypted_disk_with_sanitized_state() {
        let dir = tempdir().unwrap();
        let settings = AppSettings {
            api_key: "sk-encrypted-recovery".to_string(),
            groq_api_key: "gsk-encrypted-recovery".to_string(),
            ..sample_settings()
        };
        let disk =
            serde_json::to_string_pretty(&DiskSettings::from_settings(&settings).unwrap()).unwrap();
        fs::write(dir.path().join("settings.json"), disk).unwrap();
        let store = Arc::new(MockCredentialStore::default());
        store.unavailable.store(true, Ordering::SeqCst);

        let state = SettingsState::load_with_store(dir.path().to_path_buf(), store);
        let loaded = state.settings.lock().unwrap().clone();
        let mut frontend = loaded.to_frontend();
        state.apply_credential_state(&mut frontend);

        assert_eq!(loaded.api_key, "sk-encrypted-recovery");
        assert_eq!(loaded.groq_api_key, "gsk-encrypted-recovery");
        assert_eq!(frontend.credential_storage.mode, "encrypted_disk_fallback");
        let message = frontend.credential_storage.message.unwrap();
        assert!(!message.contains("sk-encrypted-recovery"));
        assert!(!message.contains("gsk-encrypted-recovery"));
    }

    #[cfg(windows)]
    #[test]
    fn test_existing_keyring_value_is_authoritative_over_encrypted_fallback() {
        let dir = tempdir().unwrap();
        let disk_settings = AppSettings {
            api_key: "sk-disk-value".to_string(),
            groq_api_key: "gsk-disk-value".to_string(),
            ..sample_settings()
        };
        fs::write(
            dir.path().join("settings.json"),
            serde_json::to_string_pretty(&DiskSettings::from_settings(&disk_settings).unwrap())
                .unwrap(),
        )
        .unwrap();
        let store = Arc::new(MockCredentialStore::with_secrets(&[
            (OPENAI_API_KEY_ACCOUNT, "sk-keyring-value"),
            (GROQ_API_KEY_ACCOUNT, "gsk-keyring-value"),
        ]));

        let state = SettingsState::load_with_store(dir.path().to_path_buf(), store);
        let loaded = state.settings.lock().unwrap();

        assert_eq!(loaded.api_key, "sk-keyring-value");
        assert_eq!(loaded.groq_api_key, "gsk-keyring-value");
    }

    #[test]
    fn test_partial_keyring_write_rolls_back_and_reports_failure() {
        let dir = tempdir().unwrap();
        let store = Arc::new(MockCredentialStore::with_secrets(&[
            (OPENAI_API_KEY_ACCOUNT, "sk-old"),
            (GROQ_API_KEY_ACCOUNT, "gsk-old"),
        ]));
        let state = SettingsState::load_with_store(dir.path().to_path_buf(), store.clone());
        *store.fail_write_account.lock().unwrap() = Some(GROQ_API_KEY_ACCOUNT.to_string());

        let error = state
            .save_request(SaveSettingsRequest {
                api_key: Some("sk-new".to_string()),
                groq_api_key: Some("gsk-new".to_string()),
                ..sample_save_request()
            })
            .err()
            .expect("partial credential write must fail");

        assert_eq!(
            store.secret(OPENAI_API_KEY_ACCOUNT).as_deref(),
            Some("sk-old")
        );
        assert_eq!(
            store.secret(GROQ_API_KEY_ACCOUNT).as_deref(),
            Some("gsk-old")
        );
        assert_eq!(state.settings.lock().unwrap().api_key, "sk-old");
        let disk = fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(!disk.contains("sk-new"));
        assert!(!disk.contains("gsk-new"));
        assert!(!error.contains("sk-old"));
        assert!(!error.contains("sk-new"));
        assert!(error.contains("No settings were changed"));
    }

    #[test]
    fn test_settings_recovers_last_known_good_atomic_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let storage = crate::persistence::AtomicFile::new(path.clone());
        let first = DiskSettings {
            language: "pt".to_string(),
            ..DiskSettings::default()
        };
        let second = DiskSettings {
            language: "en".to_string(),
            ..DiskSettings::default()
        };
        storage
            .write(
                storage.reserve_revision(),
                serde_json::to_string_pretty(&first).unwrap().as_bytes(),
            )
            .unwrap();
        storage
            .write(
                storage.reserve_revision(),
                serde_json::to_string_pretty(&second).unwrap().as_bytes(),
            )
            .unwrap();
        fs::write(&path, "partial-json").unwrap();

        let state = SettingsState::load_with_store(
            dir.path().to_path_buf(),
            Arc::new(MockCredentialStore::default()),
        );

        assert_eq!(state.settings.lock().unwrap().language, "pt");
        assert!(serde_json::from_str::<DiskSettings>(&fs::read_to_string(path).unwrap()).is_ok());
    }

    #[test]
    fn test_load_normalizes_legacy_prompt_optimizer_model_to_supported_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "model": "gpt-4o-mini-transcribe",
  "language": "auto",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "prompt_optimization_enabled": true,
  "prompt_optimizer_model": "gpt-5.4-nano",
  "replacements": []
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();

        assert_eq!(settings.model, "gpt-transcribe");
        assert_eq!(settings.prompt_optimizer_model, "gpt-5.4-mini");

        let migrated_json = fs::read_to_string(path).unwrap();
        assert!(migrated_json.contains(r#""model": "gpt-transcribe""#));
        assert!(migrated_json.contains(r#""prompt_optimizer_model": "gpt-5.4-mini""#));
        assert!(!migrated_json.contains("gpt-4o-mini-transcribe"));
        assert!(!migrated_json.contains("gpt-5.4-nano"));
    }

    #[test]
    fn test_validate_settings_valid() {
        assert!(validate_settings(&sample_settings()).is_ok());
        assert!(validate_settings(&AppSettings {
            model: "gpt-transcribe".to_string(),
            ..sample_settings()
        })
        .is_ok());
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
    fn test_load_preserves_whisper_large_v3_groq_model() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "transcription_provider": "groq",
  "model": "whisper-large-v3",
  "language": "pt",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "widget_mode": false,
  "prompt_optimization_enabled": false,
  "prompt_optimizer_model": "gpt-5.4-mini",
  "replacements": []
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();

        assert_eq!(settings.transcription_provider, "groq");
        assert_eq!(settings.model, "whisper-large-v3");

        let migrated_json = fs::read_to_string(path).unwrap();
        assert!(migrated_json.contains(r#""model": "whisper-large-v3""#));
        assert!(!migrated_json.contains(r#""model": "whisper-large-v3-turbo""#));
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

    #[test]
    fn test_load_normalizes_blank_repaste_hotkey_and_input_device_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "model": "gpt-4o-transcribe",
  "language": "auto",
  "auto_paste": true,
  "preserve_clipboard": false,
  "hotkey": "CommandOrControl+Shift+Space",
  "repaste_hotkey": "   ",
  "widget_mode": false,
  "input_device_id": "   ",
  "mic_sensitivity": 60,
  "noise_suppression_enabled": false,
  "prompt_optimization_enabled": false,
  "prompt_optimizer_model": "gpt-5.4-mini",
  "replacements": []
}"#,
        )
        .unwrap();

        let state = test_state(&dir);
        let settings = state
            .settings
            .lock()
            .expect("Failed to acquire settings lock")
            .clone();

        assert_eq!(settings.model, "gpt-transcribe");
        assert!(settings.repaste_hotkey.is_empty());
        assert!(settings.input_device_id.is_empty());
    }

    #[test]
    fn test_validate_settings_rejects_mouse_repaste_hotkey() {
        let settings = AppSettings {
            repaste_hotkey: "Mouse4".to_string(),
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| error.contains("keyboard shortcut")));
    }

    #[test]
    fn test_validate_settings_rejects_duplicate_hotkeys() {
        let settings = AppSettings {
            repaste_hotkey: "CommandOrControl+Shift+Space".to_string(),
            ..sample_settings()
        };

        let result = validate_settings(&settings);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| error.contains("different from the recording hotkey")));
    }
}
