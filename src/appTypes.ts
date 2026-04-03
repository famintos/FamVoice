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
  input_device_id: string;
  repaste_hotkey: string;
  noise_suppression_enabled: boolean;
  widget_mode: boolean;
  mic_sensitivity: number;
  prompt_optimization_enabled: boolean;
  prompt_optimizer_model: string;
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
  input_device_id: string;
  repaste_hotkey: string;
  noise_suppression_enabled: boolean;
  widget_mode: boolean;
  mic_sensitivity: number;
  prompt_optimization_enabled: boolean;
  prompt_optimizer_model: string;
  replacements: Replacement[];
}

export interface InputDeviceOption {
  id: string;
  label: string;
  is_default: boolean;
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
