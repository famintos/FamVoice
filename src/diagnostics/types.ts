export type DiagnosticStatus = "ok" | "warning" | "error";

export interface MicrophoneSignalTest {
  status: DiagnosticStatus;
  rms: number;
  peak: number;
  signalDetected: boolean;
  sampleCount: number;
}

export interface DiagnosticsSnapshot {
  version: {
    appVersion: string;
    platform: string;
    architecture: string;
  };
  device: {
    status: DiagnosticStatus;
    selectedLabel: string | null;
    usesSystemDefault: boolean;
    connected: boolean;
    streamHealthy: boolean;
  };
  hotkey: {
    status: DiagnosticStatus;
    recordingHotkey: string;
    recordingAvailable: boolean;
    repasteHotkey: string | null;
    repasteAvailable: boolean | null;
    conflict: boolean;
  };
  provider: {
    status: DiagnosticStatus;
    provider: string;
    model: string;
    apiKeyConfigured: boolean;
    lastTest: ProviderConnectivityTest | null;
  };
  microphoneTest: MicrophoneSignalTest | null;
  lastOperation: {
    sequence: number;
    operation: "dictation" | "microphone_test" | "provider_test" | "snapshot";
    latencyMs: number;
    succeeded: boolean;
    error: string | null;
  } | null;
}

export interface ProviderConnectivityTest {
  status: DiagnosticStatus;
  provider: string;
  latencyMs: number;
  authenticated: boolean;
  error: string | null;
}
