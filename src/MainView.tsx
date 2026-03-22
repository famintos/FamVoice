import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  AlertCircle,
  Copy,
  History as HistoryIcon,
  Minus,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
  X,
} from "lucide-react";
import { WIDGET_CURSOR_POLL_INTERVAL_MS, WIDGET_DRAG_START_GRACE_MS } from "./appConstants";
import type {
  HistoryItem,
  SettingsViewModel,
  Status,
  WidgetWindowMetrics,
} from "./appTypes";
import { FamVoiceLogo } from "./FamVoiceLogo";
import { VoiceWave } from "./components/VoiceWave";
import { WidgetView } from "./WidgetView";
import {
  getWidgetInteractiveBounds,
  getWidgetWindowSizeWithChrome,
  isPointInsideBounds,
} from "./widgetSizing.js";

export function MainView() {
  const [status, setStatus] = useState<Status>("idle");
  const [transcript, setTranscript] = useState("");
  const [settings, setSettings] = useState<SettingsViewModel | null>(null);
  const [activeTab, setActiveTab] = useState<"record" | "history">("record");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [highlightKey, setHighlightKey] = useState(0);
  const widgetContainerRef = useRef<HTMLElement | null>(null);
  const lastWidgetSizeRef = useRef<{ width: number; height: number } | null>(null);
  const ignoreCursorEventsRef = useRef<boolean | null>(null);
  const widgetDragGraceUntilRef = useRef(0);
  const widgetWindowMetricsRef = useRef<WidgetWindowMetrics | null>(null);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    invoke<SettingsViewModel>("get_settings").then(setSettings);
    void loadHistory();

    const unlistenStatus = listen<Status>("status", (event) => {
      setStatus(event.payload);
    });

    const unlistenTranscript = listen<string>("transcript", (event) => {
      setTranscript(event.payload);
    });

    const unlistenSettings = listen<SettingsViewModel>("settings-updated", (event) => {
      setSettings(event.payload);
    });

    const unlistenHistory = listen<HistoryItem[]>("history-updated", (event) => {
      setHistory(event.payload);
    });

    const unlistenHighlight = listen("highlight-widget", () => {
      setHighlightKey((k) => k + 1);
    });

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenTranscript.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
      unlistenHighlight.then((fn) => fn());
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
      const size = getWidgetWindowSizeWithChrome(container.getBoundingClientRect());
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
      widgetWindowMetricsRef.current = null;
      void appWindow.setIgnoreCursorEvents(false);
      return;
    }

    let cancelled = false;

    const loadWindowMetrics = async (): Promise<WidgetWindowMetrics> => {
      const [windowPosition, scaleFactor] = await Promise.all([
        appWindow.innerPosition(),
        appWindow.scaleFactor(),
      ]);

      return { windowPosition, scaleFactor };
    };

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

      const metrics = widgetWindowMetricsRef.current ?? await loadWindowMetrics();
      widgetWindowMetricsRef.current = metrics;

      const cursor = await cursorPosition();
      const bounds = getWidgetInteractiveBounds({
        rect: container.getBoundingClientRect(),
        windowPosition: metrics.windowPosition,
        scaleFactor: metrics.scaleFactor,
      });
      const shouldProcessCursorEvents = isPointInsideBounds(cursor, bounds);

      const nextIgnoreValue = !shouldProcessCursorEvents;
      if (ignoreCursorEventsRef.current === nextIgnoreValue) {
        return;
      }

      ignoreCursorEventsRef.current = nextIgnoreValue;
      await appWindow.setIgnoreCursorEvents(nextIgnoreValue);
    };

    const syncFromWindowMove = ({ payload }: { payload: { x: number; y: number } }) => {
      widgetWindowMetricsRef.current = {
        ...(widgetWindowMetricsRef.current ?? { scaleFactor: 1 }),
        windowPosition: payload,
      };
      void syncCursorInteractivity();
    };

    const syncFromScaleChange = ({ payload }: { payload: { scaleFactor: number } }) => {
      widgetWindowMetricsRef.current = {
        ...(widgetWindowMetricsRef.current ?? { windowPosition: { x: 0, y: 0 } }),
        scaleFactor: payload.scaleFactor,
      };
      void syncCursorInteractivity();
    };

    void syncCursorInteractivity();
    const unlistenMoved = appWindow.onMoved(syncFromWindowMove);
    const unlistenScaleChanged = appWindow.onScaleChanged(syncFromScaleChange);
    const intervalId = window.setInterval(() => {
      void syncCursorInteractivity();
    }, WIDGET_CURSOR_POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
      ignoreCursorEventsRef.current = null;
      widgetWindowMetricsRef.current = null;
      unlistenMoved.then((fn) => fn());
      unlistenScaleChanged.then((fn) => fn());
      void appWindow.setIgnoreCursorEvents(false);
    };
  }, [appWindow, settings?.widget_mode]);

  useEffect(() => {
    check()
      .then(async (update) => {
        if (!update) return;
        console.log(`Update available: ${update.version}`);
        await update.downloadAndInstall();
        setPendingUpdate(update);
      })
      .catch((error) => {
        console.error("Update check failed:", error);
      });
  }, []);

  const loadHistory = async () => {
    const items = await invoke<HistoryItem[]>("get_history");
    setHistory(items);
  };

  const handleOpenSettings = async () => {
    await invoke("open_settings_window");
  };

  const handleUpdate = async () => {
    await relaunch();
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

  const missingTranscriptionKey = settings && (
    (settings.transcription_provider === "groq" && !settings.groq_api_key_present) ||
    (settings.transcription_provider === "openai" && !settings.api_key_present)
  );

  const missingAnthropicKey = settings && settings.prompt_optimization_enabled && !settings.anthropic_api_key_present;

  if (settings?.widget_mode) {
    return (
      <WidgetView
        status={status}
        missingApiKey={!!missingTranscriptionKey}
        updateReady={!!pendingUpdate}
        highlightKey={highlightKey}
        errorMessage={status === "error" ? transcript : undefined}
        containerRef={widgetContainerRef}
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
          if (pendingUpdate) {
            void handleUpdate();
          } else {
            void handleOpenSettings();
          }
        }}
      />
    );
  }

  return (
    <main data-tauri-drag-region className="w-full h-full flex flex-col bg-[#0f0f13]/85 backdrop-blur-2xl rounded-2xl shadow-2xl border border-white/10 relative overflow-hidden text-white">
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
              <div className="mt-4 px-3 py-2 bg-black/40 backdrop-blur-sm rounded-lg border border-white/5 text-[11px] text-gray-300 w-full shadow-inner text-center animate-in fade-in slide-in-from-bottom-2 duration-300">
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

            {status === "idle" && !transcript && pendingUpdate && (
              <button
                onClick={handleUpdate}
                className="mt-4 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg text-[11px] text-green-300 cursor-pointer hover:bg-green-500/20 transition-all no-drag animate-in fade-in duration-300 w-full"
              >
                v{pendingUpdate.version} ready — click to restart
              </button>
            )}

            {status === "idle" && !transcript && (missingTranscriptionKey || missingAnthropicKey) && (
              <div className="mt-4 flex flex-col gap-2 w-full no-drag animate-in fade-in duration-300">
                {missingTranscriptionKey && (
                  <button
                    onClick={handleOpenSettings}
                    className="px-3 py-2 bg-amber-500/10 border border-amber-500/20 rounded-lg text-[11px] text-amber-300 cursor-pointer hover:bg-amber-500/20 transition-all"
                  >
                    {settings.transcription_provider === "groq" ? "Groq" : "OpenAI"} key missing
                  </button>
                )}
                {missingAnthropicKey && (
                  <button
                    onClick={handleOpenSettings}
                    className="px-3 py-2 bg-amber-500/10 border border-amber-500/20 rounded-lg text-[11px] text-amber-300 cursor-pointer hover:bg-amber-500/20 transition-all"
                  >
                    Anthropic key missing
                  </button>
                )}
              </div>
            )}

            {status === "idle" && !transcript && !missingTranscriptionKey && !missingAnthropicKey && (
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
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
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
