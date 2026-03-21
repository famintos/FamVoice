export type Status = "idle" | "recording" | "transcribing" | "success" | "error";

export interface Replacement {
  target: string;
  replacement: string;
}

export interface SettingsViewModel {
  transcription_provider: string;
  api_key_present: boolean;
  api_key_masked: string | null;
  groq_api_key_present: boolean;
  groq_api_key_masked: string | null;
  model: string;
  language: string;
  auto_paste: boolean;
  preserve_clipboard: boolean;
  hotkey: string;
  widget_mode: boolean;
  mic_sensitivity: number;
  prompt_optimization_enabled: boolean;
  prompt_optimizer_model: string;
  anthropic_api_key_present: boolean;
  anthropic_api_key_masked: string | null;
  replacements: Replacement[];
}

export interface SaveSettingsPayload {
  transcription_provider: string;
  api_key: string | null;
  groq_api_key: string | null;
  model: string;
  language: string;
  auto_paste: boolean;
  preserve_clipboard: boolean;
  hotkey: string;
  widget_mode: boolean;
  mic_sensitivity: number;
  prompt_optimization_enabled: boolean;
  prompt_optimizer_model: string;
  anthropic_api_key: string | null;
  replacements: Replacement[];
}

export interface HistoryItem {
  id: number;
  text: string;
  timestamp: number;
}

export interface WidgetWindowMetrics {
  windowPosition: { x: number; y: number };
  scaleFactor: number;
}
