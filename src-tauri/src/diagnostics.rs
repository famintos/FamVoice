use crate::audio::{InputDeviceOption, InputSignalLevels};
use crate::settings::AppSettings;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri_plugin_global_shortcut::Shortcut;

const DIAGNOSTICS_SCHEMA_VERSION: u8 = 1;
const PROVIDER_TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOperation {
    Dictation,
    MicrophoneTest,
    ProviderTest,
    Snapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastOperationDiagnostic {
    pub sequence: u64,
    pub operation: DiagnosticOperation,
    pub latency_ms: u64,
    pub succeeded: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneSignalTest {
    pub status: DiagnosticStatus,
    pub rms: f64,
    pub peak: f64,
    pub signal_detected: bool,
    pub sample_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDiagnostic {
    pub status: DiagnosticStatus,
    pub selected_label: Option<String>,
    pub uses_system_default: bool,
    pub connected: bool,
    pub stream_healthy: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyDiagnostic {
    pub status: DiagnosticStatus,
    pub recording_hotkey: String,
    pub recording_available: bool,
    pub repaste_hotkey: Option<String>,
    pub repaste_available: Option<bool>,
    pub conflict: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectivityTest {
    pub status: DiagnosticStatus,
    pub provider: String,
    pub latency_ms: u64,
    pub authenticated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnostic {
    pub status: DiagnosticStatus,
    pub provider: String,
    pub model: String,
    pub api_key_configured: bool,
    pub last_test: Option<ProviderConnectivityTest>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiagnostic {
    pub app_version: String,
    pub platform: String,
    pub architecture: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    pub version: VersionDiagnostic,
    pub device: DeviceDiagnostic,
    pub hotkey: HotkeyDiagnostic,
    pub provider: ProviderDiagnostic,
    pub microphone_test: Option<MicrophoneSignalTest>,
    pub last_operation: Option<LastOperationDiagnostic>,
}

// Keep this export DTO deliberately separate from runtime state. Adding a secret,
// transcript, device identifier or PCM buffer to runtime state cannot implicitly
// make it into an exported report.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsExport<'a> {
    schema_version: u8,
    version: &'a VersionDiagnostic,
    device: ExportedDeviceDiagnostic,
    hotkey: ExportedHotkeyDiagnostic,
    provider: &'a ProviderDiagnostic,
    microphone_test: &'a Option<MicrophoneSignalTest>,
    last_operation: &'a Option<LastOperationDiagnostic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedDeviceDiagnostic {
    status: DiagnosticStatus,
    uses_system_default: bool,
    connected: bool,
    stream_healthy: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedHotkeyDiagnostic {
    status: DiagnosticStatus,
    recording_available: bool,
    repaste_enabled: bool,
    repaste_available: Option<bool>,
    conflict: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticsRuntimeSnapshot {
    pub microphone_test: Option<MicrophoneSignalTest>,
    pub provider_test: Option<ProviderConnectivityTest>,
    pub last_operation: Option<LastOperationDiagnostic>,
}

#[derive(Debug, Default)]
pub struct DiagnosticsState {
    next_sequence: AtomicU64,
    runtime: Mutex<DiagnosticsRuntimeSnapshot>,
}

#[derive(Debug)]
pub struct DiagnosticOperationToken {
    sequence: u64,
    operation: DiagnosticOperation,
    started_at: Instant,
}

impl DiagnosticsState {
    pub fn begin_operation(&self, operation: DiagnosticOperation) -> DiagnosticOperationToken {
        DiagnosticOperationToken {
            sequence: self.next_sequence.fetch_add(1, Ordering::SeqCst) + 1,
            operation,
            started_at: Instant::now(),
        }
    }

    pub fn finish_operation(
        &self,
        token: DiagnosticOperationToken,
        result: Result<(), &str>,
    ) -> LastOperationDiagnostic {
        self.finish_operation_at(token, result, Instant::now())
    }

    fn finish_operation_at(
        &self,
        token: DiagnosticOperationToken,
        result: Result<(), &str>,
        finished_at: Instant,
    ) -> LastOperationDiagnostic {
        let operation = LastOperationDiagnostic {
            sequence: token.sequence,
            operation: token.operation,
            latency_ms: duration_millis_saturated(
                finished_at.saturating_duration_since(token.started_at),
            ),
            succeeded: result.is_ok(),
            error: result.err().map(sanitize_error),
        };

        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let can_replace = runtime
            .last_operation
            .as_ref()
            .is_none_or(|current| operation.sequence >= current.sequence);
        if can_replace {
            runtime.last_operation = Some(operation.clone());
        }
        operation
    }

    pub fn record_microphone_test(&self, test: MicrophoneSignalTest) {
        self.runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .microphone_test = Some(test);
    }

    pub fn record_provider_test(&self, test: ProviderConnectivityTest) {
        self.runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .provider_test = Some(test);
    }

    pub fn snapshot(&self) -> DiagnosticsRuntimeSnapshot {
        self.runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

pub fn sanitize_error(error: &str) -> String {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("unauthorized")
        || normalized.contains("authentication")
        || normalized.contains("api key")
        || normalized.contains("credential")
    {
        "Authentication failed.".to_string()
    } else if normalized.contains("rate limit")
        || normalized.contains("quota")
        || normalized.contains("429")
    {
        "The provider rate limit or quota was reached.".to_string()
    } else if normalized.contains("timeout") || normalized.contains("timed out") {
        "The operation timed out.".to_string()
    } else if normalized.contains("network")
        || normalized.contains("connect")
        || normalized.contains("reach")
    {
        "The network is unavailable.".to_string()
    } else if normalized.contains("microphone")
        || normalized.contains("input device")
        || normalized.contains("audio stream")
    {
        "The microphone is unavailable.".to_string()
    } else if normalized.contains("hotkey")
        || normalized.contains("shortcut")
        || normalized.contains("conflict")
    {
        "The hotkey is unavailable.".to_string()
    } else if normalized.contains("provider")
        || normalized.contains("server")
        || normalized.contains("service unavailable")
    {
        "The provider is temporarily unavailable.".to_string()
    } else {
        "The operation failed.".to_string()
    }
}

pub fn microphone_test_from_levels(levels: &InputSignalLevels) -> MicrophoneSignalTest {
    let signal_detected = levels.peak >= 0.01 || levels.rms >= 0.005;
    MicrophoneSignalTest {
        status: if levels.sample_count == 0 {
            DiagnosticStatus::Error
        } else if signal_detected {
            DiagnosticStatus::Ok
        } else {
            DiagnosticStatus::Warning
        },
        rms: levels.rms,
        peak: levels.peak,
        signal_detected,
        sample_count: levels.sample_count,
    }
}

pub fn device_snapshot(
    selected_device_id: &str,
    devices: &[InputDeviceOption],
    stream_healthy: bool,
) -> DeviceDiagnostic {
    let normalized_selection = selected_device_id.trim();
    let uses_system_default = normalized_selection.is_empty();
    let selected = if uses_system_default {
        devices
            .iter()
            .find(|device| device.is_default)
            .or_else(|| devices.first())
    } else {
        devices
            .iter()
            .find(|device| device.id == normalized_selection)
    };
    let connected = selected.is_some();
    let selected_label = selected.map(export_safe_device_label);
    let status = if !connected {
        DiagnosticStatus::Error
    } else if stream_healthy {
        DiagnosticStatus::Ok
    } else {
        DiagnosticStatus::Warning
    };

    DeviceDiagnostic {
        status,
        selected_label,
        uses_system_default,
        connected,
        stream_healthy,
    }
}

fn export_safe_device_label(device: &InputDeviceOption) -> String {
    let label = device.label.trim();
    if label.is_empty() {
        return "Unknown microphone".to_string();
    }
    if device.id.is_empty() {
        return label.to_string();
    }
    label.replace(&device.id, "redacted")
}

pub fn hotkey_snapshot(
    recording_hotkey: &str,
    repaste_hotkey: &str,
    recording_registered: bool,
    repaste_registered: bool,
) -> HotkeyDiagnostic {
    let recording = recording_hotkey.trim();
    let repaste = repaste_hotkey.trim();
    let recording_valid = hotkey_syntax_valid(recording);
    let repaste_enabled = !repaste.is_empty();
    let repaste_valid = !repaste_enabled || hotkey_syntax_valid(repaste);
    let conflict = repaste_enabled && recording.eq_ignore_ascii_case(repaste);
    let recording_available = recording_valid && recording_registered && !conflict;
    let repaste_available =
        repaste_enabled.then_some(repaste_valid && repaste_registered && !conflict);
    let status = if !recording_valid || !repaste_valid || conflict {
        DiagnosticStatus::Error
    } else if !recording_available || repaste_available == Some(false) {
        DiagnosticStatus::Warning
    } else {
        DiagnosticStatus::Ok
    };

    HotkeyDiagnostic {
        status,
        recording_hotkey: recording.to_string(),
        recording_available,
        repaste_hotkey: repaste_enabled.then(|| repaste.to_string()),
        repaste_available,
        conflict,
    }
}

fn hotkey_syntax_valid(hotkey: &str) -> bool {
    !hotkey.is_empty()
        && (crate::input_hook::is_mouse_hotkey(hotkey) || hotkey.parse::<Shortcut>().is_ok())
}

pub fn provider_snapshot(
    settings: &AppSettings,
    last_test: Option<ProviderConnectivityTest>,
) -> ProviderDiagnostic {
    let provider = normalized_provider_name(&settings.transcription_provider);
    let api_key_configured = !settings.transcription_api_key().trim().is_empty();
    let status = if !api_key_configured {
        DiagnosticStatus::Error
    } else if let Some(test) = &last_test {
        test.status
    } else {
        DiagnosticStatus::Warning
    };

    ProviderDiagnostic {
        status,
        provider,
        model: settings.model.clone(),
        api_key_configured,
        last_test,
    }
}

pub fn version_snapshot() -> VersionDiagnostic {
    VersionDiagnostic {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
    }
}

pub fn diagnostics_snapshot(
    settings: &AppSettings,
    devices: &[InputDeviceOption],
    stream_healthy: bool,
    recording_hotkey_registered: bool,
    repaste_hotkey_registered: bool,
    state: &DiagnosticsState,
) -> DiagnosticsSnapshot {
    let runtime = state.snapshot();
    DiagnosticsSnapshot {
        version: version_snapshot(),
        device: device_snapshot(&settings.input_device_id, devices, stream_healthy),
        hotkey: hotkey_snapshot(
            &settings.hotkey,
            &settings.repaste_hotkey,
            recording_hotkey_registered,
            repaste_hotkey_registered,
        ),
        provider: provider_snapshot(settings, runtime.provider_test),
        microphone_test: runtime.microphone_test,
        last_operation: runtime.last_operation,
    }
}

pub fn export_diagnostics_json(snapshot: &DiagnosticsSnapshot) -> Result<String, String> {
    serde_json::to_string_pretty(&DiagnosticsExport {
        schema_version: DIAGNOSTICS_SCHEMA_VERSION,
        version: &snapshot.version,
        device: ExportedDeviceDiagnostic {
            status: snapshot.device.status,
            uses_system_default: snapshot.device.uses_system_default,
            connected: snapshot.device.connected,
            stream_healthy: snapshot.device.stream_healthy,
        },
        hotkey: ExportedHotkeyDiagnostic {
            status: snapshot.hotkey.status,
            recording_available: snapshot.hotkey.recording_available,
            repaste_enabled: snapshot.hotkey.repaste_hotkey.is_some(),
            repaste_available: snapshot.hotkey.repaste_available,
            conflict: snapshot.hotkey.conflict,
        },
        provider: &snapshot.provider,
        microphone_test: &snapshot.microphone_test,
        last_operation: &snapshot.last_operation,
    })
    .map_err(|_| "Failed to serialize diagnostics export.".to_string())
}

pub async fn test_provider_models(
    client: &reqwest::Client,
    provider: &str,
    api_key: &str,
) -> ProviderConnectivityTest {
    let provider_name = normalized_provider_name(provider);
    let endpoint = match provider_models_endpoint(provider) {
        Some(endpoint) => endpoint,
        None => {
            return ProviderConnectivityTest {
                status: DiagnosticStatus::Error,
                provider: provider_name,
                latency_ms: 0,
                authenticated: false,
                error: Some("Unsupported transcription provider.".to_string()),
            };
        }
    };

    test_provider_models_at_endpoint(client, &provider_name, api_key, endpoint).await
}

async fn test_provider_models_at_endpoint(
    client: &reqwest::Client,
    provider_name: &str,
    api_key: &str,
    endpoint: &str,
) -> ProviderConnectivityTest {
    if api_key.trim().is_empty() {
        return ProviderConnectivityTest {
            status: DiagnosticStatus::Error,
            provider: provider_name.to_string(),
            latency_ms: 0,
            authenticated: false,
            error: Some("No API key is configured.".to_string()),
        };
    }

    let started_at = Instant::now();
    let response = client
        .get(endpoint)
        .bearer_auth(api_key)
        .timeout(PROVIDER_TEST_TIMEOUT)
        .send()
        .await;
    let latency_ms = duration_millis_saturated(started_at.elapsed());

    match response {
        Ok(response) if response.status().is_success() => ProviderConnectivityTest {
            status: DiagnosticStatus::Ok,
            provider: provider_name.to_string(),
            latency_ms,
            authenticated: true,
            error: None,
        },
        Ok(response) => ProviderConnectivityTest {
            status: if response.status().is_server_error() {
                DiagnosticStatus::Warning
            } else {
                DiagnosticStatus::Error
            },
            provider: provider_name.to_string(),
            latency_ms,
            authenticated: false,
            error: Some(provider_status_error(response.status())),
        },
        Err(error) => ProviderConnectivityTest {
            status: DiagnosticStatus::Warning,
            provider: provider_name.to_string(),
            latency_ms,
            authenticated: false,
            error: Some(if error.is_timeout() {
                "The provider test timed out.".to_string()
            } else {
                "Could not reach the provider.".to_string()
            }),
        },
    }
}

fn provider_models_endpoint(provider: &str) -> Option<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => Some("https://api.openai.com/v1/models"),
        "groq" => Some("https://api.groq.com/openai/v1/models"),
        _ => None,
    }
}

fn normalized_provider_name(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => "OpenAI".to_string(),
        "groq" => "Groq".to_string(),
        _ => "Unsupported".to_string(),
    }
}

fn provider_status_error(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "Provider authentication failed.".to_string()
        }
        StatusCode::TOO_MANY_REQUESTS => {
            "The provider rate limit or quota was reached.".to_string()
        }
        status if status.is_server_error() => {
            "The provider is temporarily unavailable.".to_string()
        }
        _ => format!("Provider test failed with HTTP {}.", status.as_u16()),
    }
}

fn duration_millis_saturated(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn test_settings() -> AppSettings {
        AppSettings {
            transcription_provider: "openai".to_string(),
            api_key: "sk-diagnostic-secret-sentinel".to_string(),
            groq_api_key: "gsk-diagnostic-secret-sentinel".to_string(),
            model: "whisper-1".to_string(),
            hotkey: "CommandOrControl+Shift+Space".to_string(),
            repaste_hotkey: "CommandOrControl+Shift+V".to_string(),
            input_device_id: "device-id-secret-sentinel".to_string(),
            ..AppSettings::default()
        }
    }

    #[test]
    fn signal_levels_are_reduced_to_statistics_without_pcm() {
        let test = microphone_test_from_levels(&InputSignalLevels {
            rms: 0.04,
            peak: 0.25,
            sample_count: 24_000,
        });
        let serialized = serde_json::to_string(&test).unwrap();

        assert_eq!(test.status, DiagnosticStatus::Ok);
        assert!(test.signal_detected);
        assert!(!serialized.contains("samples"));
        assert!(!serialized.contains("pcm"));
    }

    #[test]
    fn device_snapshot_detects_a_disconnected_selection_without_exporting_its_id() {
        let diagnostic = device_snapshot(
            "missing-device-id",
            &[InputDeviceOption {
                id: "other-device-id".to_string(),
                label: "Desk microphone".to_string(),
                is_default: true,
            }],
            false,
        );
        let serialized = serde_json::to_string(&diagnostic).unwrap();

        assert_eq!(diagnostic.status, DiagnosticStatus::Error);
        assert!(!diagnostic.connected);
        assert!(!serialized.contains("missing-device-id"));
        assert!(!serialized.contains("other-device-id"));
    }

    #[test]
    fn hotkey_snapshot_reports_conflicts() {
        let diagnostic = hotkey_snapshot(
            "CommandOrControl+Shift+Space",
            "CommandOrControl+Shift+Space",
            true,
            true,
        );

        assert_eq!(diagnostic.status, DiagnosticStatus::Error);
        assert!(diagnostic.conflict);
        assert!(!diagnostic.recording_available);
        assert_eq!(diagnostic.repaste_available, Some(false));
    }

    #[test]
    fn a_slower_older_operation_cannot_overwrite_a_newer_result() {
        let state = DiagnosticsState::default();
        let older = state.begin_operation(DiagnosticOperation::ProviderTest);
        let newer = state.begin_operation(DiagnosticOperation::MicrophoneTest);

        let newer_result = state.finish_operation(newer, Ok(()));
        let older_result = state.finish_operation(older, Err("sk-secret transcript sentinel"));
        let runtime = state.snapshot();

        assert!(newer_result.sequence > older_result.sequence);
        assert_eq!(runtime.last_operation, Some(newer_result));
    }

    #[test]
    fn sanitized_errors_never_echo_arbitrary_content_or_tokens() {
        let sentinel = "sk-secret transcript personal-content device-id-42";
        let sanitized = sanitize_error(sentinel);

        assert_eq!(sanitized, "The operation failed.");
        assert!(!sanitized.contains(sentinel));
        assert!(!sanitized.contains("sk-secret"));
    }

    #[test]
    fn export_is_an_allowlist_without_keys_transcripts_or_device_ids() {
        let settings = test_settings();
        let state = DiagnosticsState::default();
        let operation = state.begin_operation(DiagnosticOperation::ProviderTest);
        state.finish_operation(
            operation,
            Err("transcript-personal-sentinel device-id-secret-sentinel sk-diagnostic-secret-sentinel"),
        );
        let devices = vec![InputDeviceOption {
            id: settings.input_device_id.clone(),
            label: format!("Microphone {}", settings.input_device_id),
            is_default: false,
        }];
        let snapshot = diagnostics_snapshot(&settings, &devices, true, true, true, &state);
        let exported = export_diagnostics_json(&snapshot).unwrap();

        assert!(exported.contains("\"schemaVersion\": 1"));
        assert!(exported.contains("\"apiKeyConfigured\": true"));
        assert!(!exported.contains("sk-diagnostic-secret-sentinel"));
        assert!(!exported.contains("gsk-diagnostic-secret-sentinel"));
        assert!(!exported.contains("transcript-personal-sentinel"));
        assert!(!exported.contains("device-id-secret-sentinel"));
        assert!(!exported.contains("Microphone"));
        assert!(!exported.contains("CommandOrControl"));
        assert!(!exported.contains("api_key"));
        assert!(!exported.contains("transcript"));
    }

    #[tokio::test]
    async fn provider_test_uses_authenticated_get_models_without_a_request_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                )
                .unwrap();
            request
        });
        let endpoint = format!("http://{address}/models");
        let api_key = "sk-provider-test-sentinel";

        let result =
            test_provider_models_at_endpoint(&reqwest::Client::new(), "OpenAI", api_key, &endpoint)
                .await;
        let request = server.join().unwrap();
        let request_lowercase = request.to_ascii_lowercase();

        assert_eq!(result.status, DiagnosticStatus::Ok);
        assert!(result.authenticated);
        assert!(request.starts_with("GET /models HTTP/1.1\r\n"));
        assert!(request_lowercase.contains("authorization: bearer sk-provider-test-sentinel"));
        assert!(!request_lowercase.contains("content-length:"));
        assert!(request.ends_with("\r\n\r\n"));

        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains(api_key));
    }

    #[tokio::test]
    async fn provider_http_errors_are_sanitized_without_reading_response_content() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 57\r\nConnection: close\r\n\r\nsk-response-secret transcript-personal-sentinel device-id-42",
                )
                .unwrap();
        });

        let result = test_provider_models_at_endpoint(
            &reqwest::Client::new(),
            "OpenAI",
            "sk-request-secret",
            &format!("http://{address}/models"),
        )
        .await;
        server.join().unwrap();
        let serialized = serde_json::to_string(&result).unwrap();

        assert_eq!(result.status, DiagnosticStatus::Error);
        assert_eq!(
            result.error.as_deref(),
            Some("Provider authentication failed.")
        );
        assert!(!serialized.contains("sk-response-secret"));
        assert!(!serialized.contains("transcript-personal-sentinel"));
        assert!(!serialized.contains("device-id-42"));
        assert!(!serialized.contains("sk-request-secret"));
    }
}
