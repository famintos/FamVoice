import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize, PhysicalSize } from "@tauri-apps/api/window";

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
  replacements: Replacement[];
}

interface HistoryItem {
  id: number;
  text: string;
  timestamp: number;
}


function VoiceWave({ isPlaying = false }: { isPlaying?: boolean }) {
  return (
    <div className="flex items-center justify-center h-4 gap-[2px] pointer-events-none">
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          className={`wave-bar w-[3px] bg-primary rounded-full h-full ${!isPlaying ? "pause-animation" : ""}`}
          style={{ height: isPlaying ? undefined : "40%" }}
        />
      ))}
    </div>
  );
}

const DEFAULT_HOTKEY = "CommandOrControl+Shift+Space";

const LANGUAGES = [
  { value: "auto", label: "Auto Detect" },
  { value: "en", label: "English" },
  { value: "pt", label: "Portuguese" },
  { value: "es", label: "Spanish" },
  { value: "fr", label: "French" },
  { value: "de", label: "German" },
  { value: "it", label: "Italian" },
  { value: "nl", label: "Dutch" },
  { value: "ja", label: "Japanese" },
  { value: "ko", label: "Korean" },
  { value: "zh", label: "Chinese" },
  { value: "ru", label: "Russian" },
  { value: "ar", label: "Arabic" },
  { value: "hi", label: "Hindi" },
  { value: "pl", label: "Polish" },
  { value: "sv", label: "Swedish" },
  { value: "da", label: "Danish" },
  { value: "no", label: "Norwegian" },
  { value: "fi", label: "Finnish" },
  { value: "tr", label: "Turkish" },
  { value: "uk", label: "Ukrainian" },
  { value: "cs", label: "Czech" },
  { value: "ro", label: "Romanian" },
  { value: "hu", label: "Hungarian" },
  { value: "el", label: "Greek" },
  { value: "he", label: "Hebrew" },
  { value: "th", label: "Thai" },
  { value: "vi", label: "Vietnamese" },
  { value: "id", label: "Indonesian" },
  { value: "ms", label: "Malay" },
];

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


  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
    isEnabled().then(setAutostart);
  }, []);

  const saveSettings = async (newSettings: Settings) => {
    try {
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
      <main className="w-full h-full flex items-center justify-center bg-[#0f0f13] text-gray-400 text-xs">
        Loading settings...
      </main>
    );
  }

  return (
    <main className="w-full h-full flex flex-col p-4 bg-[#0f0f13] text-white overflow-hidden border border-white/10 rounded-xl">
      <div className="flex items-center justify-between mb-4 select-none" data-tauri-drag-region>
        <div className="flex items-center gap-2 pointer-events-none">
          <SettingsIcon size={14} className="text-primary" />
          <h2 className="text-sm font-bold tracking-wide">Settings</h2>
        </div>
        <button onClick={() => invoke("close_settings_window")} className="p-1 hover:bg-white/10 rounded cursor-pointer transition-colors">
          <X size={16} className="text-gray-400 hover:text-white" />
        </button>
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
              Language
              <select
                value={settings.language}
                onChange={e => setSettings({ ...settings, language: e.target.value })}
                className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
              >
                {LANGUAGES.map(lang => (
                  <option key={lang.value} value={lang.value}>{lang.label}</option>
                ))}
              </select>
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
              <span>Preserve Clipboard</span>
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
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Replacements</h3>
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
                  placeholder="find"
                  className="flex-1 min-w-0 p-1.5 bg-black/40 rounded border border-white/10 text-[10px] text-white focus:outline-none focus:border-primary transition-colors"
                />
                <span className="text-gray-500 text-[10px]">→</span>
                <input
                  value={rep.replacement}
                  onChange={e => updateReplacement(i, "replacement", e.target.value)}
                  placeholder="replace"
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
              <p className="text-[10px] text-gray-600 italic">No replacements configured.</p>
            )}
          </div>
        </section>
      </div>

      <div className="pt-4 border-t border-white/5 mt-auto">
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
  const [showWidgetMenu, setShowWidgetMenu] = useState(false);
  const appWindow = getCurrentWindow();




  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
    loadHistory();

    const unlistenStatus = listen<Status>("status", (event) => {
      setStatus(event.payload);
      if (event.payload === "success") {
        loadHistory();
      }
    });

    const unlistenTranscript = listen<string>("transcript", (event) => {
      setTranscript(event.payload);
    });

    const unlistenSettings = listen<Settings>("settings-updated", (event) => {
      setSettings(event.payload);
    });

    return () => {
      unlistenStatus.then(f => f());
      unlistenTranscript.then(f => f());
      unlistenSettings.then(f => f());
    };
  }, []);

  useEffect(() => {
    const updateSize = async () => {
      if (settings) {
        await appWindow.setResizable(true);
        // Clear constraints first so the resize isn't blocked
        await appWindow.setMinSize(new PhysicalSize(1, 1));
        await appWindow.setMaxSize(new PhysicalSize(9999, 9999));

        if (settings.widget_mode) {
          // Handled by ResizeObserver now, but set a base starting size
          const size = new PhysicalSize(160, 44);
          await appWindow.setSize(size);
        } else {
          const defaultSize = new LogicalSize(260, 200);
          await appWindow.setSize(defaultSize);
          await appWindow.setMinSize(defaultSize);
          await appWindow.setMaxSize(defaultSize);
        }
        await appWindow.setResizable(false);
        await appWindow.setMaximizable(false);
        await appWindow.center();
      }
    };
    updateSize();
  }, [settings?.widget_mode]);

  useEffect(() => {
    if (!settings?.widget_mode) return;
    
    const container = document.getElementById("widget-container");
    if (!container) return;

    const observer = new ResizeObserver(async (entries) => {
      for (let entry of entries) {
        // Add a small margin (e.g., 2px) to prevent clipping if needed
        const width = Math.ceil(entry.contentRect.width) + 8;
        const height = Math.ceil(entry.contentRect.height) + 8;
        
        const size = new PhysicalSize(width, height);
        await appWindow.setMinSize(new PhysicalSize(1, 1));
        await appWindow.setMaxSize(new PhysicalSize(9999, 9999));
        await appWindow.setSize(size);
        await appWindow.setMinSize(size);
        await appWindow.setMaxSize(size);
      }
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, [settings?.widget_mode, status, showWidgetMenu]);

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
    loadHistory();
  };

  const clearHistory = async () => {
    await invoke("clear_history");
    setHistory([]);
  };

  if (settings?.widget_mode) {
    return (
      <div className="w-full h-full flex items-center justify-center" style={{ pointerEvents: "none" }}>
        <main
          id="widget-container"
          data-tauri-drag-region
          className="relative flex items-center gap-3 px-4 py-2 bg-[#0f0f13] backdrop-blur-2xl rounded-full shadow-md border border-white/10 text-white"
          style={{ pointerEvents: "auto" }}
          onContextMenu={(e) => {
            e.preventDefault();
            setShowWidgetMenu(prev => !prev);
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

          {showWidgetMenu && (
            <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 bg-[#1a1a24] border border-white/10 rounded-lg shadow-2xl overflow-hidden min-w-[120px]">
              <button
                onClick={() => { setShowWidgetMenu(false); handleOpenSettings(); }}
                className="flex items-center gap-2 w-full px-3 py-2 text-[11px] text-gray-300 hover:bg-white/10 hover:text-white transition-colors no-drag cursor-pointer"
              >
                <SettingsIcon size={12} />
                Settings
              </button>
            </div>
          )}
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
              {status === "recording" ? (
                <VoiceWave />
              ) : (
                <div className={`w-3 h-3 rounded-full transition-all duration-500 ${status === "idle" ? "bg-gray-700 shadow-none" :
                  status === "transcribing" ? "bg-yellow-500 animate-pulse shadow-[0_0_15px_rgba(234,179,8,0.4)]" :
                    status === "success" ? "bg-green-500 shadow-[0_0_15px_rgba(34,197,94,0.4)]" :
                      "bg-red-500 shadow-[0_0_15px_rgba(239,68,68,0.4)]"
                  }`} />
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