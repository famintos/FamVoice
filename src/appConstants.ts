export const DEFAULT_HOTKEY = "CommandOrControl+Shift+Space";

export const TRANSCRIPTION_PROVIDERS = [
  { value: "openai", label: "OpenAI" },
  { value: "groq", label: "Groq" },
  { value: "soniox", label: "Soniox" },
  { value: "openrouter", label: "OpenRouter" },
];

export const DEFAULT_TRANSCRIPTION_MODEL_BY_PROVIDER: Record<string, string> = {
  openai: "gpt-transcribe",
  groq: "whisper-large-v3-turbo",
  soniox: "stt-rt-v5",
  openrouter: "microsoft/mai-transcribe-2",
};

export const OPENAI_MODELS = [
  { value: "gpt-transcribe", label: "gpt-transcribe — Recommended" },
  { value: "whisper-1", label: "whisper-1 — Specialized fallback" },
];

export const GROQ_MODELS = [
  { value: "whisper-large-v3-turbo", label: "whisper-large-v3-turbo (Fast)" },
  { value: "whisper-large-v3", label: "whisper-large-v3 (Accuracy)" },
];

export const MODELS_BY_PROVIDER: Record<string, typeof OPENAI_MODELS> = {
  openai: OPENAI_MODELS,
  groq: GROQ_MODELS,
  soniox: [{ value: "stt-rt-v5", label: "Soniox v5" }],
  openrouter: [
    { value: "microsoft/mai-transcribe-2", label: "MAI Transcribe 2" },
    { value: "elevenlabs/scribe-v2", label: "Scribe v2" },
  ],
};

export const TRANSCRIPTION_MODEL_HELP: Record<string, string> = {
  "elevenlabs/scribe-v2":
    "Portuguese and English dictation with your saved OpenRouter key. Try Auto Detect for mixed speech.",
  "microsoft/mai-transcribe-2":
    "Transcribes your dictation without prompt optimization. Inserts text without sending it.",
  "stt-rt-v5":
    "Multilingual dictation with vocabulary guidance. Choose Portuguese for European Portuguese with English terms. Estimated Soniox usage: $0.12/hour.",
  "gpt-transcribe":
    "Recommended for completed dictation. Supports language and vocabulary guidance, prompts, and streamed file responses. OpenAI list price: $0.0045/min.",
  "whisper-1":
    "Specialized fallback for word timestamps, SRT/VTT subtitles, or translation to English. File response streaming is unavailable. OpenAI list price: $0.006/min.",
  "whisper-large-v3-turbo":
    "Groq's faster, lower-cost multilingual option for everyday dictation ($0.04/hour).",
  "whisper-large-v3":
    "Groq's accuracy-first multilingual option for error-sensitive dictation ($0.111/hour).",
};

export const LANGUAGES = [
  { value: "auto", label: "Auto Detect" },
  { value: "ar", label: "Arabic" },
  { value: "de", label: "German" },
  { value: "en", label: "English" },
  { value: "es", label: "Spanish" },
  { value: "fr", label: "French" },
  { value: "hi", label: "Hindi" },
  { value: "it", label: "Italian" },
  { value: "ja", label: "Japanese" },
  { value: "ko", label: "Korean" },
  { value: "nl", label: "Dutch" },
  { value: "pl", label: "Polish" },
  { value: "pt", label: "Portuguese" },
  { value: "ru", label: "Russian" },
  { value: "tr", label: "Turkish" },
  { value: "uk", label: "Ukrainian" },
  { value: "zh", label: "Chinese" },
];

export const PROMPT_OPTIMIZER_MODELS = [
  { value: "gpt-5.4-mini", label: "GPT-5.4 Mini" },
];

export const WIDGET_DRAG_START_GRACE_MS = 180;
export const WIDGET_CURSOR_POLL_INTERVAL_MS = 75;
