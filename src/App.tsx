import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";

import {
  Settings as SettingsIcon,
  Minus,
  X,
  History as HistoryIcon,
  Copy,
  RefreshCw,
  Trash2,
  Plus,
  AlertCircle
} from "lucide-react";
import "./App.css";
import { FamVoiceLogo } from "./FamVoiceLogo";
import {
  getWidgetInteractiveBounds,
  getWidgetWindowSize,
  isPointInsideBounds,
} from "./widgetSizing.js";

type Status = "idle" | "recording" | "transcribing" | "success" | "error";

interface Replacement {
  target: string;
  replacement: string;
}

interface Settings {
  api_key: string;
  model: string;
  language: string;
  auto_paste: boolean;
  preserve_clipboard: boolean;
  hotkey: string;
  widget_mode: boolean;
  mic_sensitivity: number;
  prompt_optimization_enabled: boolean;
  prompt_optimizer_provider: string;
  prompt_optimizer_model: string;
  anthropic_api_key: string;
  replacements: Replacement[];
}

interface HistoryItem {
  id: number;
  text: string;
  timestamp: number;
}


function VoiceWave({
  isPlaying = false,
  size = "default",
}: {
  isPlaying?: boolean;
  size?: "default" | "large";
}) {
  const containerClass = size === "large"
    ? "h-8 gap-1"
    : "h-4 gap-[2px]";
  const barClass = size === "large" ? "w-[4px]" : "w-[3px]";
  const pausedHeight = size === "large" ? "55%" : "40%";

  return (
    <div className={`flex items-center justify-center ${containerClass} pointer-events-none`}>
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          className={`wave-bar ${barClass} bg-primary rounded-full h-full ${!isPlaying ? "pause-animation" : ""}`}
          style={{ height: isPlaying ? undefined : pausedHeight }}
        />
      ))}
    </div>
  );
}

const DEFAULT_HOTKEY = "CommandOrControl+Shift+Space";

const LANGUAGES = [
  { value: "auto", label: "Auto Detect" },
  { value: "pt", label: "Portuguese" },
  { value: "en", label: "English" },
];

const PROMPT_OPTIMIZER_PROVIDERS = [
  { value: "anthropic", label: "Anthropic" },
];

const PROMPT_OPTIMIZER_MODELS = [
  { value: "claude-haiku-4-5", label: "Claude Haiku 4.5" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
];

const WIDGET_DRAG_START_GRACE_MS = 180;

function isInteractiveDragTarget(target: EventTarget | null): boolean {
  return target instanceof Element
    && target.closest("button, input, select, textarea, a, [role='button'], [contenteditable='true'], .no-drag") !== null;
}

function buildHotkeyString(e: React.KeyboardEvent): string | null {
  const key = e.key;
  // Ignore lone modifier presses
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  // Map special keys to Tauri shortcut tokens
  const keyMap: Record<string, string> = {
    " ": "Space",
    ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
    Enter: "Enter", Escape: "Escape", Backspace: "Backspace", Tab: "Tab",
    Delete: "Delete", Insert: "Insert", Home: "Home", End: "End",
    PageUp: "PageUp", PageDown: "PageDown",
  };

  const mainKey = keyMap[key] ?? (key.length === 1 ? key.toUpperCase() : key);
  parts.push(mainKey);

  return parts.join("+");
}

function formatHotkey(hotkey: string): string {
  if (hotkey === "Mouse3") return "Mouse 3 (Middle)";
  if (hotkey === "Mouse4") return "Mouse 4 (Back)";
  if (hotkey === "Mouse5") return "Mouse 5 (Forward)";

  return hotkey
    .replace("CommandOrControl", "Ctrl")
    .replace(/\+/g, " + ");
}


function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [isListening, setIsListening] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const appWindow = getCurrentWindow();


  useEffect(() => {
    invoke<Settings>("get_settings")
      .then(setSettings)
      .catch((error) => {
        console.error("Failed to load settings:", error);
        setErrorMessage(String(error));
      });
    isEnabled()
      .then(setAutostart)
      .catch((error) => {
        console.error("Autostart status error:", error);
      });
  }, []);

  const saveSettings = async (newSettings: Settings) => {
    try {
      setErrorMessage(null);
      await invoke("save_settings", { newSettings });
      setSettings(newSettings);
      try {
        if (autostart) await enable(); else await disable();
      } catch (e) {
        console.error("Autostart error:", e);
      }
      await invoke("close_settings_window");
    } catch (error) {
      console.error("Failed to save settings:", error);
      setErrorMessage(String(error));
    }
  };

  const addReplacement = () => {
    if (!settings) return;
    setSettings({
      ...settings,
      replacements: [...settings.replacements, { target: "", replacement: "" }]
    });
  };

  const removeReplacement = (index: number) => {
    if (!settings) return;
    const newReps = [...settings.replacements];
    newReps.splice(index, 1);
    setSettings({ ...settings, replacements: newReps });
  };

  const updateReplacement = (index: number, field: keyof Replacement, value: string) => {
    if (!settings) return;
    const newReps = [...settings.replacements];
    newReps[index] = { ...newReps[index], [field]: value };
    setSettings({ ...settings, replacements: newReps });
  };

  const handleMouseCapture = (e: React.MouseEvent<HTMLInputElement>) => {
    if (!isListening || !settings) return;

    // Tauri/Browser button map: 0=Left, 1=Middle, 2=Right, 3=Back(M4), 4=Forward(M5)
    // We only want to capture 1 (Middle), 3 (Mouse4), 4 (Mouse5)
    if (e.button === 1 || e.button === 3 || e.button === 4) {
      e.preventDefault();
      const combo = e.button === 1 ? "Mouse3" : e.button === 3 ? "Mouse4" : "Mouse5";
      setSettings({ ...settings, hotkey: combo });
      setIsListening(false);
      (e.target as HTMLInputElement).blur();
    }
  };

  const handleHotkeyCapture = (e: React.KeyboardEvent<HTMLInputElement>) => {

    e.preventDefault();
    e.stopPropagation();
    const combo = buildHotkeyString(e);
    if (combo && settings) {
      setSettings({ ...settings, hotkey: combo });
      setIsListening(false);
      (e.target as HTMLInputElement).blur();
    }
  };

  if (!settings) {
    return (
      <main className="w-full h-full flex items-center justify-center bg-[#0f0f13] px-4 text-center text-xs">
        <div className={errorMessage ? "text-red-300" : "text-gray-400"}>
          {errorMessage ?? "Loading settings..."}
        </div>
      </main>
    );
  }

  return (
    <main className="w-full h-full flex flex-col p-4 bg-[#0f0f13] text-white overflow-hidden border border-white/10 rounded-xl">
      <div
        className="-mx-4 -mt-4 mb-2 px-4 pt-4 pb-3 select-none"
        onMouseDownCapture={(e) => {
          if (e.button !== 0 || isInteractiveDragTarget(e.target)) return;
          e.preventDefault();
          void appWindow.startDragging().catch((error) => {
            console.error("Failed to start settings drag:", error);
          });
        }}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 pointer-events-none">
            <SettingsIcon size={14} className="text-primary" />
            <h2 className="text-sm font-bold tracking-wide">Settings</h2>
          </div>
          <button onClick={() => invoke("close_settings_window")} className="p-1 hover:bg-white/10 rounded cursor-pointer transition-colors">
            <X size={16} className="text-gray-400 hover:text-white" />
          </button>
        </div>
      </div>

      <div className="flex-1 flex flex-col gap-5 overflow-y-auto overflow-x-hidden pr-2 custom-scrollbar pb-4">
        <section className="space-y-3">
          <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">General</h3>
          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            OpenAI API Key
            <input
              type="password"
              value={settings.api_key}
              onChange={e => setSettings({ ...settings, api_key: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full"
              placeholder="sk-..."
            />
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Model
            <select
              value={settings.model}
              onChange={e => setSettings({ ...settings, model: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              <option value="gpt-4o-mini-transcribe">gpt-4o-mini-transcribe</option>
              <option value="gpt-4o-transcribe">gpt-4o-transcribe (High Accuracy)</option>
              <option value="whisper-1">whisper-1 (Legacy / Fallback)</option>
            </select>
          </label>
        </section>

        <section className="space-y-3">
          <div className="flex flex-col gap-1">
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Prompt Optimization</h3>
            <span className="text-[10px] text-gray-500">
              Runs a second Anthropic pass after transcription to rewrite the final transcript into a prompt.
            </span>
          </div>

          <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
            <input
              type="checkbox"
              checked={settings.prompt_optimization_enabled}
              onChange={e => setSettings({ ...settings, prompt_optimization_enabled: e.target.checked })}
              className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
            />
            <div className="flex flex-col">
              <span>Improve into prompt</span>
              <span className="text-[10px] text-gray-500">Adds an extra Anthropic model pass that rewrites the finalized transcript into a prompt.</span>
            </div>
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Provider
            <select
              value={settings.prompt_optimizer_provider}
              onChange={e => setSettings({ ...settings, prompt_optimizer_provider: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              {PROMPT_OPTIMIZER_PROVIDERS.map(provider => (
                <option key={provider.value} value={provider.value}>{provider.label}</option>
              ))}
            </select>
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Model
            <select
              value={settings.prompt_optimizer_model}
              onChange={e => setSettings({ ...settings, prompt_optimizer_model: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              {PROMPT_OPTIMIZER_MODELS.map(model => (
                <option key={model.value} value={model.value}>{model.label}</option>
              ))}
            </select>
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Anthropic API Key
            <input
              type="password"
              value={settings.anthropic_api_key}
              onChange={e => setSettings({ ...settings, anthropic_api_key: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full"
              placeholder="sk-ant-..."
            />
          </label>
        </section>

        <section className="space-y-3">
          <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Behavior</h3>
          <div className="flex flex-col gap-3">
            <div className="text-xs text-gray-400 flex flex-col gap-1.5">
              Hotkey
              <div className="flex gap-2 items-center">
                <input
                  type="text"
                  readOnly
                  value={isListening ? "Press keys..." : formatHotkey(settings.hotkey)}
                  onFocus={() => setIsListening(true)}
                  onBlur={() => setIsListening(false)}
                  onKeyDown={handleHotkeyCapture}
                  onMouseDown={handleMouseCapture}
                  onContextMenu={(e) => isListening && e.preventDefault()}
                  className={`flex-1 p-2 bg-black/40 rounded border text-xs text-white focus:outline-none transition-colors w-full cursor-pointer ${isListening ? "border-primary bg-primary/10" : "border-white/10"
                    }`}
                />
                <button
                  type="button"
                  onClick={() => setSettings({ ...settings, hotkey: DEFAULT_HOTKEY })}
                  title="Reset to default"
                  className="p-2 bg-black/40 rounded border border-white/10 text-gray-400 hover:text-primary hover:border-primary/50 transition-colors cursor-pointer"
                >
                  <RefreshCw size={12} />
                </button>
              </div>
            </div>
            <label className="text-xs text-gray-400 flex flex-col gap-1.5">
              Language Preference
              <select
                value={settings.language}
                onChange={e => setSettings({ ...settings, language: e.target.value })}
                className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
              >
                {LANGUAGES.map(lang => (
                  <option key={lang.value} value={lang.value}>{lang.label}</option>
                ))}
              </select>
              <span className="text-[10px] text-gray-500">
                Auto Detect handles mixed dictation. Choose Portuguese or English only if you want to bias transcription toward one language.
              </span>
            </label>

            <label className="text-xs text-gray-400 flex flex-col gap-1.5">
              Mic Sensitivity
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={settings.mic_sensitivity}
                  onChange={e => setSettings({ ...settings, mic_sensitivity: Number(e.target.value) })}
                  className="flex-1 accent-primary cursor-pointer"
                />
                <span className="w-8 text-right text-[10px] text-gray-500">
                  {settings.mic_sensitivity}
                </span>
              </div>
              <div className="flex justify-between text-[10px] text-gray-600">
                <span>Less noise</span>
                <span>Quieter voice</span>
              </div>
              <span className="text-[10px] text-gray-500">
                Higher sensitivity helps softer speech, but can pick up more background noise.
              </span>
            </label>
          </div>

          <div className="space-y-2">
            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.widget_mode}
                onChange={e => setSettings({ ...settings, widget_mode: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Widget Mode</span>
                <span className="text-[10px] text-gray-500">Minimal UI with only waveforms</span>
              </div>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.auto_paste}
                onChange={e => setSettings({ ...settings, auto_paste: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <span>Auto Paste Transcript</span>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.preserve_clipboard}
                onChange={e => setSettings({ ...settings, preserve_clipboard: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Preserve Clipboard</span>
                <span className="text-[10px] text-gray-500">Restore the original clipboard after a successful auto-paste</span>
              </div>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={autostart}
                onChange={e => setAutostart(e.target.checked)}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <span>Launch on Startup</span>
            </label>
          </div>
        </section>

        <section className="space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Glossary</h3>
            <button
              onClick={addReplacement}
              className="text-[10px] text-primary hover:text-blue-400 flex items-center gap-1 cursor-pointer"
            >
              <Plus size={10} /> Add
            </button>
          </div>

          <div className="space-y-2">
            {settings.replacements.map((rep, i) => (
              <div key={i} className="flex gap-2 items-center">
                <input
                  value={rep.target}
                  onChange={e => updateReplacement(i, "target", e.target.value)}
                  placeholder="spoken term"
                  className="flex-1 min-w-0 p-1.5 bg-black/40 rounded border border-white/10 text-[10px] text-white focus:outline-none focus:border-primary transition-colors"
                />
                <span className="text-gray-500 text-[10px]">→</span>
                <input
                  value={rep.replacement}
                  onChange={e => updateReplacement(i, "replacement", e.target.value)}
                  placeholder="preferred text"
                  className="flex-1 min-w-0 p-1.5 bg-black/40 rounded border border-white/10 text-[10px] text-white focus:outline-none focus:border-primary transition-colors"
                />
                <button
                  onClick={() => removeReplacement(i)}
                  className="text-gray-600 hover:text-red-400 p-1 cursor-pointer"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
            {settings.replacements.length === 0 && (
              <p className="text-[10px] text-gray-600 italic">No glossary entries configured.</p>
            )}
          </div>
        </section>
      </div>

      <div className="pt-4 border-t border-white/5 mt-auto">
        {errorMessage && (
          <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
            {errorMessage}
          </div>
        )}
        <button
          onClick={() => saveSettings(settings)}
          className="w-full bg-primary hover:bg-blue-600 py-2.5 rounded text-xs font-bold cursor-pointer transition-all shadow-lg active:scale-[0.98]"
        >
          Save Changes
        </button>
      </div>
    </main>
  );
}

function MainView() {
  const [status, setStatus] = useState<Status>("idle");
  const [transcript, setTranscript] = useState("");

  const [settings, setSettings] = useState<Settings | null>(null);
  const [activeTab, setActiveTab] = useState<"record" | "history">("record");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const widgetContainerRef = useRef<HTMLElement | null>(null);
  const lastWidgetSizeRef = useRef<{ width: number; height: number } | null>(null);
  const ignoreCursorEventsRef = useRef<boolean | null>(null);
  const widgetDragGraceUntilRef = useRef(0);
  const appWindow = getCurrentWindow();




  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
    loadHistory();

    const unlistenStatus = listen<Status>("status", (event) => {
      setStatus(event.payload);
    });

    const unlistenTranscript = listen<string>("transcript", (event) => {
      setTranscript(event.payload);
    });

    const unlistenSettings = listen<Settings>("settings-updated", (event) => {
      setSettings(event.payload);
    });

    const unlistenHistory = listen<HistoryItem[]>("history-updated", (event) => {
      setHistory(event.payload);
    });

    return () => {
      unlistenStatus.then(f => f());
      unlistenTranscript.then(f => f());
      unlistenSettings.then(f => f());
      unlistenHistory.then(f => f());
    };
  }, []);

  useEffect(() => {
    if (!settings?.widget_mode) {
      lastWidgetSizeRef.current = null;
      return;
    }

    const container = widgetContainerRef.current;
    if (!container) return;

    let frameId = 0;
    const resizeWindow = async () => {
      const size = getWidgetWindowSize(container.getBoundingClientRect());
      const previousSize = lastWidgetSizeRef.current;

      if (previousSize?.width === size.width && previousSize?.height === size.height) {
        return;
      }

      lastWidgetSizeRef.current = { width: size.width, height: size.height };
      await invoke("resize_main_window", { width: size.width, height: size.height });
    };

    const scheduleResize = () => {
      cancelAnimationFrame(frameId);
      frameId = requestAnimationFrame(() => {
        void resizeWindow();
      });
    };

    const observer = new ResizeObserver(() => {
      scheduleResize();
    });

    observer.observe(container);
    scheduleResize();

    return () => {
      cancelAnimationFrame(frameId);
      observer.disconnect();
      lastWidgetSizeRef.current = null;
    };
  }, [settings?.widget_mode]);

  useEffect(() => {
    if (!settings?.widget_mode) {
      ignoreCursorEventsRef.current = null;
      void appWindow.setIgnoreCursorEvents(false);
      return;
    }

    let cancelled = false;

    const syncCursorInteractivity = async () => {
      if (cancelled) return;

      const container = widgetContainerRef.current;
      if (!container) return;

      if (Date.now() < widgetDragGraceUntilRef.current) {
        if (ignoreCursorEventsRef.current === false) {
          return;
        }

        ignoreCursorEventsRef.current = false;
        await appWindow.setIgnoreCursorEvents(false);
        return;
      }

      const shouldProcessCursorEvents = await (async () => {
        const [cursor, windowPosition, scaleFactor] = await Promise.all([
          cursorPosition(),
          appWindow.innerPosition(),
          appWindow.scaleFactor(),
        ]);

        const bounds = getWidgetInteractiveBounds({
          rect: container.getBoundingClientRect(),
          windowPosition,
          scaleFactor,
        });

        return isPointInsideBounds(cursor, bounds);
      })();

      const nextIgnoreValue = !shouldProcessCursorEvents;
      if (ignoreCursorEventsRef.current === nextIgnoreValue) {
        return;
      }

      ignoreCursorEventsRef.current = nextIgnoreValue;
      await appWindow.setIgnoreCursorEvents(nextIgnoreValue);
    };

    void syncCursorInteractivity();
    const intervalId = window.setInterval(() => {
      void syncCursorInteractivity();
    }, 16);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
      ignoreCursorEventsRef.current = null;
      void appWindow.setIgnoreCursorEvents(false);
    };
  }, [appWindow, settings?.widget_mode]);

  const loadHistory = async () => {
    const h = await invoke<HistoryItem[]>("get_history");
    setHistory(h);
  };

  const handleOpenSettings = async () => {
    await invoke("open_settings_window");
  };

  const copyToClipboard = async (text: string) => {
    await navigator.clipboard.writeText(text);
  };

  const repasteHistory = async (text: string) => {
    await invoke("repaste_history_item", { text });
  };

  const deleteHistory = async (id: number) => {
    await invoke("delete_history_item", { id });
  };

  const clearHistory = async () => {
    await invoke("clear_history");
  };

  const showStatusDot = status === "transcribing" || status === "success" || status === "error";

  if (settings?.widget_mode) {
    return (
      <div className="w-full h-full flex items-center justify-center" style={{ pointerEvents: "none" }}>
        <main
          ref={widgetContainerRef}
          id="widget-container"
          className="relative flex items-center gap-3 px-4 py-2 bg-[#0f0f13] backdrop-blur-2xl rounded-md shadow-md border border-white/10 text-white"
          style={{ pointerEvents: "auto" }}
          onMouseDownCapture={(e) => {
            if (e.button !== 0) return;
            e.preventDefault();
            widgetDragGraceUntilRef.current = Date.now() + WIDGET_DRAG_START_GRACE_MS;
            ignoreCursorEventsRef.current = false;
            void appWindow.setIgnoreCursorEvents(false);
            void appWindow.startDragging().catch((error) => {
              console.error("Failed to start widget drag:", error);
            });
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            void handleOpenSettings();
          }}
        >
          <div className="flex items-center gap-2 pointer-events-none select-none">
            <div className="flex items-center justify-center shadow-[0_0_15px_rgba(255,81,47,0.4)] rounded-full">
              <FamVoiceLogo size={24} />
            </div>
            <span className="text-[11px] font-bold tracking-wider uppercase opacity-90">Fam</span>
          </div>

          <div className="flex items-center gap-1.5 pointer-events-none">
            {status === "transcribing" && (
              <div className="w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
            )}
            {status === "error" && (
              <div className="w-2 h-2 bg-red-500 rounded-full" />
            )}
            <VoiceWave isPlaying={status === "recording"} />
          </div>

        </main>
      </div>
    );
  }

  return (
    <main data-tauri-drag-region className="w-full h-full flex flex-col bg-[#0f0f13]/85 backdrop-blur-2xl rounded-2xl shadow-2xl border border-white/10 relative overflow-hidden text-white">
      {/* Top Bar */}
      <div data-tauri-drag-region className="flex justify-between items-center px-4 pt-3 pb-1">
        <div className="flex items-center gap-2 pointer-events-none select-none text-gray-300">
          <FamVoiceLogo size={16} />
          <span className="text-[11px] font-bold tracking-[0.1em] uppercase">FamVoice</span>
        </div>

        <div className="flex items-center gap-1.5">
          <button
            onClick={handleOpenSettings}
            className="text-gray-400 hover:text-white cursor-pointer no-drag p-1.5 rounded hover:bg-white/10 transition-all"
          >
            <SettingsIcon size={14} />
          </button>
          <button
            onClick={() => appWindow.minimize()}
            className="text-gray-400 hover:text-white cursor-pointer no-drag p-1.5 rounded hover:bg-white/10 transition-all"
          >
            <Minus size={14} />
          </button>
          <button
            onClick={() => appWindow.close()}
            className="text-gray-400 hover:text-red-400 cursor-pointer no-drag p-1.5 rounded hover:bg-white/10 transition-all"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex px-4 gap-4 border-b border-white/5 no-drag select-none">
        <button
          onClick={() => setActiveTab("record")}
          className={`pb-2 text-[10px] font-bold uppercase tracking-wider transition-all cursor-pointer relative ${activeTab === "record" ? "text-primary" : "text-gray-500 hover:text-gray-300"
            }`}
        >
          Dictate
          {activeTab === "record" && <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary rounded-full" />}
        </button>
        <button
          onClick={() => setActiveTab("history")}
          className={`pb-2 text-[10px] font-bold uppercase tracking-wider transition-all cursor-pointer relative ${activeTab === "history" ? "text-primary" : "text-gray-500 hover:text-gray-300"
            }`}
        >
          History
          {activeTab === "history" && <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary rounded-full" />}
        </button>
      </div>

      <div className="flex-1 overflow-hidden">
        {activeTab === "record" ? (
          <div data-tauri-drag-region className="h-full flex flex-col items-center justify-center p-6 relative">
            <div className="flex items-center justify-center h-12 mb-2 pointer-events-none">
              {showStatusDot ? (
                <div className={`w-3 h-3 rounded-full transition-all duration-500 ${status === "transcribing" ? "bg-yellow-500 animate-pulse shadow-[0_0_15px_rgba(234,179,8,0.4)]" :
                  status === "success" ? "bg-green-500 shadow-[0_0_15px_rgba(34,197,94,0.4)]" :
                    "bg-red-500 shadow-[0_0_15px_rgba(239,68,68,0.4)]"
                  }`} />
              ) : (
                <VoiceWave isPlaying={status === "recording"} size="large" />
              )}
            </div>

            <p className="text-xs text-gray-400 select-none text-center pointer-events-none font-medium tracking-wide">
              {status === "idle" ? "" :
                status === "recording" ? "Listening..." :
                  status === "transcribing" ? "Transcribing..." :
                    status === "success" ? "Success!" :
                      "Error occurred"}
            </p>

            {transcript && (
              <div className="mt-4 px-3 py-2 bg-black/40 backdrop-blur-sm rounded-lg border border-white/5 text-[11px] text-gray-300 w-full max-h-20 overflow-y-auto custom-scrollbar shadow-inner text-center animate-in fade-in slide-in-from-bottom-2 duration-300">
                {status === "error" ? (
                  <div className="flex items-center justify-center gap-1.5 text-red-400">
                    <AlertCircle size={12} />
                    <span>{transcript}</span>
                  </div>
                ) : (
                  transcript
                )}
              </div>
            )}

            {status === "idle" && !transcript && (
              <div className="mt-8 flex flex-col items-center gap-2 opacity-20 pointer-events-none">
                <p className="text-[10px] uppercase tracking-widest font-bold">Ready</p>
              </div>
            )}
          </div>
        ) : (
          <div className="h-full flex flex-col no-drag">
            <div className="flex justify-between items-center p-3 px-4 border-b border-white/5">
              <span className="text-[10px] text-gray-500 uppercase font-bold tracking-wider">{history.length} items</span>
              {history.length > 0 && (
                <button
                  onClick={clearHistory}
                  className="text-[10px] text-gray-500 hover:text-red-400 flex items-center gap-1 transition-colors cursor-pointer"
                >
                  <Trash2 size={10} /> Clear
                </button>
              )}
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar p-2 px-3 space-y-2 pb-4">
              {history.map((item) => (
                <div key={item.id} className="group p-2.5 bg-white/5 hover:bg-white/10 rounded-xl border border-white/5 transition-all animate-in fade-in duration-200">
                  <p className="text-[11px] text-gray-200 line-clamp-2 mb-2 leading-relaxed">{item.text}</p>
                  <div className="flex justify-between items-center">
                    <span className="text-[9px] text-gray-600 font-medium">
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                    <div className="flex gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={() => copyToClipboard(item.text)}
                        className="p-1.5 hover:bg-white/10 rounded-lg text-gray-400 hover:text-primary transition-all cursor-pointer"
                        title="Copy"
                      >
                        <Copy size={12} />
                      </button>
                      <button
                        onClick={() => repasteHistory(item.text)}
                        className="p-1.5 hover:bg-white/10 rounded-lg text-gray-400 hover:text-green-400 transition-all cursor-pointer"
                        title="Re-paste"
                      >
                        <RefreshCw size={12} />
                      </button>
                      <button
                        onClick={() => deleteHistory(item.id)}
                        className="p-1.5 hover:bg-white/10 rounded-lg text-gray-400 hover:text-red-400 transition-all cursor-pointer"
                        title="Delete"
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
              {history.length === 0 && (
                <div className="h-full flex flex-col items-center justify-center py-12 opacity-30 pointer-events-none">
                  <HistoryIcon size={32} className="mb-2" />
                  <p className="text-[10px] uppercase tracking-widest font-bold">No history yet</p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}

function App() {
  const params = new URLSearchParams(window.location.search);
  const view = params.get("view");

  if (view === "settings") {
    return <SettingsView />;
  }

  return <MainView />;
}

export default App;
