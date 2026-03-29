import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { check, type Update } from "@tauri-apps/plugin-updater";
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

const appWindow = getCurrentWindow();

export function MainView() {
  const [status, setStatus] = useState<Status>("idle");
  const [transcript, setTranscript] = useState("");
  const [settings, setSettings] = useState<SettingsViewModel | null>(null);
  const [activeTab, setActiveTab] = useState<"record" | "history">("record");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [isUpdateNoticeOpen, setIsUpdateNoticeOpen] = useState(false);
  const [highlightKey, setHighlightKey] = useState(0);
  const widgetContainerRef = useRef<HTMLElement | null>(null);
  const lastWidgetSizeRef = useRef<{ width: number; height: number } | null>(null);
  const ignoreCursorEventsRef = useRef<boolean | null>(null);
  const widgetDragGraceUntilRef = useRef(0);
  const widgetWindowMetricsRef = useRef<WidgetWindowMetrics | null>(null);
  const lastCursorPositionRef = useRef<{ x: number; y: number } | null>(null);
  const hasDismissedUpdateNoticeRef = useRef(false);

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
      lastCursorPositionRef.current = null;
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
      const lastCursor = lastCursorPositionRef.current;
      if (lastCursor && lastCursor.x === cursor.x && lastCursor.y === cursor.y) {
        return;
      }
      lastCursorPositionRef.current = { x: cursor.x, y: cursor.y };

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
      lastCursorPositionRef.current = null;
      void syncCursorInteractivity();
    };

    const syncFromScaleChange = ({ payload }: { payload: { scaleFactor: number } }) => {
      widgetWindowMetricsRef.current = {
        ...(widgetWindowMetricsRef.current ?? { windowPosition: { x: 0, y: 0 } }),
        scaleFactor: payload.scaleFactor,
      };
      lastCursorPositionRef.current = null;
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
      lastCursorPositionRef.current = null;
      unlistenMoved.then((fn) => fn());
      unlistenScaleChanged.then((fn) => fn());
      void appWindow.setIgnoreCursorEvents(false);
    };
  }, [settings?.widget_mode]);

  useEffect(() => {
    check()
      .then((update) => {
        if (!update) return;
        console.log(`Update available: ${update.version}`);
        setPendingUpdate(update);
        if (!hasDismissedUpdateNoticeRef.current) {
          setIsUpdateNoticeOpen(true);
        }
      })
      .catch((error) => {
        console.error("Update check failed:", error);
      });
  }, []);

  const loadHistory = async () => {
    const items = await invoke<HistoryItem[]>("get_history");
    setHistory(items);
  };

  const dismissUpdateNotice = () => {
    hasDismissedUpdateNoticeRef.current = true;
    setIsUpdateNoticeOpen(false);
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

  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";

  const missingTranscriptionKey = settings && (
    (settings.transcription_provider === "groq" && !settings.groq_api_key_present) ||
    (settings.transcription_provider === "openai" && !settings.api_key_present)
  );

  const missingPromptOptimizerKey = settings && settings.prompt_optimization_enabled && !settings.api_key_present;
  const statusLabel = status === "recording"
    ? "Listening"
    : status === "transcribing"
      ? "Transcribing"
      : status === "success"
        ? "Transcript ready"
        : status === "error"
          ? "Error"
          : "Ready";
          
  const stageHint = status === "recording"
    ? "Release hotkey to send."
    : status === "transcribing"
      ? "Processing..."
      : status === "success"
        ? "Ready for paste-back."
        : status === "error"
          ? "See details below."
          : "Hold hotkey to dictate.";

  if (settings?.widget_mode) {
    return (
      <WidgetView
        status={status}
        missingApiKey={!!missingTranscriptionKey}
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
          void handleOpenSettings();
        }}
      />
    );
  }

  return (
    <main
      data-tauri-drag-region
      className="signal-shell relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[20px] bg-[#161B26]"
    >
      {pendingUpdate && isUpdateNoticeOpen && (
        <div className="absolute inset-x-2 top-2 z-20 no-drag rounded-xl bg-transparent p-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
              <p className="text-xs font-medium text-white">Update Available</p>
              <p className="text-[10px] text-primary">v{pendingUpdate.version}</p>
            </div>
            <button
              onClick={dismissUpdateNotice}
              className="text-slate-500 hover:text-white"
              aria-label="Dismiss update notice"
            >
              <X size={14} />
            </button>
          </div>
          <button
            onClick={() => {
              dismissUpdateNotice();
              void handleOpenSettings();
            }}
            className="mt-3 w-full rounded py-1.5 text-[11px] text-primary hover:text-white transition-colors text-left"
          >
            Open Settings →
          </button>
        </div>
      )}

      {/* Header */}
      <div data-tauri-drag-region className="relative z-10 flex items-center justify-between px-4 pt-4 pb-2">
        <div className="flex items-center gap-2 pointer-events-none select-none">
          <FamVoiceLogo size={18} />
          <div className="flex items-baseline font-medium text-sm text-white tracking-tight">
            FamVoice<span className="text-primary">.</span>
          </div>
        </div>

        <div className="flex items-center gap-2.5 no-drag text-slate-500">
          <button onClick={handleOpenSettings} className="hover:text-white transition-colors" title="Settings"><SettingsIcon size={12} /></button>
          <button onClick={() => appWindow.minimize()} className="hover:text-white transition-colors" title="Minimize"><Minus size={12} /></button>
          <button onClick={() => appWindow.close()} className="hover:text-red-400 transition-colors" title="Close"><X size={12} /></button>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="relative z-10 px-4 no-drag">
        <div className="flex items-center justify-between pb-2">
          <div className="flex gap-4">
            <button
              onClick={() => setActiveTab("record")}
              className={`text-[10px] font-mono uppercase tracking-widest transition-colors ${
                activeTab === "record" ? "text-primary" : "text-slate-500 hover:text-slate-300"
              }`}
            >
              Dictate
            </button>
            <button
              onClick={() => setActiveTab("history")}
              className={`text-[10px] font-mono uppercase tracking-widest transition-colors ${
                activeTab === "history" ? "text-primary" : "text-slate-500 hover:text-slate-300"
              }`}
            >
              History
            </button>
          </div>
          
          {activeTab === "history" && (
            <div className="flex items-center gap-3">
              {history.length > 0 && (
                <button
                  onClick={clearHistory}
                  className="text-[10px] font-mono uppercase tracking-widest text-slate-400 hover:text-red-400 transition-colors"
                >
                  Clear
                </button>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Content Area */}
      <div className="relative z-10 flex-1 overflow-hidden">
        {activeTab === "record" ? (
          <div className="flex h-full flex-col px-4 pb-2">
            <div className="flex flex-1 flex-col items-center justify-center text-center no-drag">
              <div className="mb-2 flex h-12 w-full items-center justify-center">
                <VoiceWave mode={waveMode} size="large" />
              </div>
              
              <h2 className="text-sm font-medium tracking-tight text-white mb-1">
                {statusLabel}
              </h2>
              
              <div className="w-full px-2 max-w-[240px]">
                {status === "error" && transcript ? (
                  <div className="flex flex-col items-center gap-1 text-red-400 text-[10px]">
                    <AlertCircle size={12} />
                    <p className="line-clamp-2 leading-tight">{transcript}</p>
                  </div>
                ) : (
                  <p className={`text-[11px] leading-tight ${transcript ? "text-slate-200 line-clamp-3" : "text-slate-500"}`}>
                    {transcript || stageHint}
                  </p>
                )}
              </div>

              {status === "idle" && !transcript && (missingTranscriptionKey || missingPromptOptimizerKey) && (
                <button
                  onClick={handleOpenSettings}
                  className="mt-6 text-[10px] font-mono uppercase tracking-widest text-primary/80 transition-colors hover:text-primary"
                >
                  Configure API Keys →
                </button>
              )}
            </div>
          </div>
        ) : (
          <div className="flex h-full flex-col no-drag">
            <div className="custom-scrollbar flex-1 overflow-y-auto px-4 pb-4">
              {history.map((item) => (
                <div key={item.id} className="group relative py-3 px-2 -mx-2 rounded-lg hover:bg-white/5 transition-colors">
                  <p className="text-xs leading-relaxed text-slate-200 pr-2">{item.text}</p>
                  <div className="mt-2 flex items-center justify-between">
                    <span className="text-[10px] text-slate-600 font-mono">
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                    <div className="flex gap-3 opacity-0 transition-opacity group-hover:opacity-100">
                      <button onClick={() => copyToClipboard(item.text)} className="text-slate-500 hover:text-white transition-colors" title="Copy">
                        <Copy size={12} />
                      </button>
                      <button onClick={() => repasteHistory(item.text)} className="text-slate-500 hover:text-primary transition-colors" title="Re-paste">
                        <RefreshCw size={12} />
                      </button>
                      <button onClick={() => deleteHistory(item.id)} className="text-slate-400 hover:text-red-400 transition-colors" title="Delete">
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
              
              {history.length === 0 && (
                <div className="flex h-full flex-col items-center justify-center text-slate-600 pointer-events-none pb-8">
                  <HistoryIcon size={24} className="mb-3 opacity-50" />
                  <p className="text-xs">No history yet</p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
