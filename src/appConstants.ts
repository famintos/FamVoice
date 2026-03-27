export const DEFAULT_HOTKEY = "CommandOrControl+Shift+Space";

export const TRANSCRIPTION_PROVIDERS = [
  { value: "openai", label: "OpenAI" },
  { value: "groq", label: "Groq" },
];

export const OPENAI_MODELS = [
  { value: "gpt-4o-mini-transcribe", label: "gpt-4o-mini-transcribe" },
  { value: "gpt-4o-transcribe", label: "gpt-4o-transcribe (High Accuracy)" },
  { value: "whisper-1", label: "whisper-1 (Legacy / Fallback)" },
];

export const GROQ_MODELS = [
  { value: "whisper-large-v3-turbo", label: "whisper-large-v3-turbo (Fast)" },
];

export const MODELS_BY_PROVIDER: Record<string, typeof OPENAI_MODELS> = {
  openai: OPENAI_MODELS,
  groq: GROQ_MODELS,
};

export const LANGUAGES = [
  { value: "auto", label: "Auto Detect" },
  { value: "pt", label: "Portuguese" },
  { value: "en", label: "English" },
];

export const PROMPT_OPTIMIZER_MODELS = [
  { value: "gpt-5.4-mini", label: "GPT-5.4 Mini" },
];

export const WIDGET_DRAG_START_GRACE_MS = 180;
export const WIDGET_CURSOR_POLL_INTERVAL_MS = 75;
