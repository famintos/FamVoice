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
import { VoiceWave } from "./components/VoiceWave";
import { FamVoiceLockup } from "./components/FamVoiceLockup";
import { WidgetView } from "./WidgetView";
import {
  getWidgetInteractiveBounds,
  getWidgetWindowSizeWithChrome,
  isPointInsideBounds,
} from "./widgetSizing.js";

const appWindow = getCurrentWindow();

export function MainView() {
  const controlMotion = "transition-colors duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)]";
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
  const showSettingsNotice = status === "idle" && !transcript && (missingTranscriptionKey || missingPromptOptimizerKey);
  const showRecordError = status === "error" && Boolean(transcript);
  const showRecordTranscript = !showRecordError && Boolean(transcript);
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
          ? "Review the message below, then try again."
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
          const target = e.target;
          if (
            target instanceof Element &&
            target.closest("button, a, input, select, textarea, [role='button']")
          ) {
            return;
          }
          e.preventDefault();
          widgetDragGraceUntilRef.current = Date.now() + WIDGET_DRAG_START_GRACE_MS;
          ignoreCursorEventsRef.current = false;
          void appWindow.setIgnoreCursorEvents(false);
          void appWindow.startDragging().catch((error) => {
            console.error("Failed to start widget drag:", error);
          });
        }}
      />
    );
  }

  return (
    <main
      className="signal-shell relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[16px] bg-[#161B26]"
    >
      {pendingUpdate && isUpdateNoticeOpen && (
        <div className="absolute inset-x-1.5 top-1.5 z-20 no-drag rounded-lg bg-transparent p-2">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
              <p className="text-xs font-medium text-white">Update available</p>
              <p className="text-xs text-primary">v{pendingUpdate.version}</p>
            </div>
            <button
              type="button"
              onClick={dismissUpdateNotice}
              className={`focus-ring rounded p-1 text-slate-500 ${controlMotion} hover:text-white`}
              aria-label="Dismiss update notice"
            >
              <X size={12} />
            </button>
          </div>
          <button
            type="button"
            onClick={() => {
              dismissUpdateNotice();
              void handleOpenSettings();
            }}
            className={`focus-ring mt-2 w-full rounded py-1 text-left text-xs font-medium text-primary ${controlMotion} hover:text-white`}
          >
            Open settings
          </button>
        </div>
      )}

      {/* Header */}
      <div data-tauri-drag-region className="relative z-10 flex items-center justify-between px-3 pt-2 pb-0.5">
        <div className="flex items-center gap-2 pointer-events-none select-none">
          <FamVoiceLockup markSize={14} motion="fade-in" />
        </div>

        <div className="flex items-center gap-1.5 no-drag text-slate-500">
          <button
            type="button"
            onClick={() => appWindow.minimize()}
            className={`focus-ring rounded p-0.5 ${controlMotion} hover:text-white`}
            aria-label="Minimize window"
          >
            <Minus size={10} />
          </button>
          <button
            type="button"
            onClick={() => appWindow.close()}
            className={`focus-ring rounded p-0.5 ${controlMotion} hover:text-red-400`}
            aria-label="Close window"
          >
            <X size={10} />
          </button>
        </div>
      </div>

      {/* Tab Switcher */}
      <div className="relative z-10 px-3 no-drag">
        <div className="flex items-center justify-between pb-0.5">
          <div className="flex gap-1" role="tablist" aria-label="Main sections">
            <button
              type="button"
              id="record-tab"
              role="tab"
              onClick={() => setActiveTab("record")}
              aria-controls="record-panel"
              aria-selected={activeTab === "record"}
               className={`focus-ring rounded-full px-2 py-1 text-[11px] font-medium tracking-tight ${controlMotion} ${
                activeTab === "record"
                  ? "bg-white/10 text-white"
                  : "text-slate-500 hover:text-slate-300"
              }`}
            >
              Record
            </button>
            <button
              type="button"
              id="history-tab"
              role="tab"
              onClick={() => setActiveTab("history")}
              aria-controls="history-panel"
              aria-selected={activeTab === "history"}
               className={`focus-ring rounded-full px-2 py-1 text-[11px] font-medium tracking-tight ${controlMotion} ${
                activeTab === "history"
                  ? "bg-white/10 text-white"
                  : "text-slate-500 hover:text-slate-300"
              }`}
            >
              History
            </button>
          </div>

          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => void handleOpenSettings()}
              className={`focus-ring inline-flex items-center gap-1 rounded-full border border-white/10 bg-black/20 px-2 py-1 text-[11px] font-medium text-slate-400 ${controlMotion} hover:border-primary/40 hover:text-white`}
              aria-label="Open settings"
            >
              <SettingsIcon size={10} />
              Settings
            </button>
            {activeTab === "history" && history.length > 0 && (
              <button
                type="button"
                onClick={clearHistory}
                className={`focus-ring rounded-full px-2 py-1 text-[11px] font-medium tracking-tight text-slate-400 ${controlMotion} hover:text-red-400`}
              >
                Clear history
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Content Area */}
      <div className="relative z-10 flex-1 min-h-0 overflow-hidden">
        {activeTab === "record" ? (
          <div
            id="record-panel"
            role="tabpanel"
            aria-labelledby="record-tab"
            className="flex h-full min-h-0 flex-col px-3 pb-3"
          >
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center rounded-[18px] border border-white/10 bg-white/[0.03] px-3 pt-1 pb-3 no-drag text-center">
              <div className="flex flex-col items-center gap-1.5">
                <VoiceWave mode={waveMode} size="large" />
                <div className="space-y-0">
                  <h2 className="text-sm font-medium tracking-tight text-white">
                    {statusLabel}
                  </h2>
                  <p
                    className={`max-w-[14rem] text-[11px] leading-tight text-slate-400 ${
                      (status === "error" || status === "success" || status === "transcribing" || Boolean(transcript)) ? "h-0 overflow-hidden" : "mt-0.5 min-h-[1.5rem]"
                    } ${
                      (status === "error" || status === "success" || status === "transcribing" || Boolean(transcript)) ? "invisible" : ""
                    }`}
                    aria-hidden={status === "error" || status === "success" || status === "transcribing" || Boolean(transcript)}
                  >
                    {stageHint}
                  </p>
                </div>
              </div>

              <div className={`${(status === "error" || status === "success" || status === "transcribing" || Boolean(transcript)) ? "mt-1.5" : "mt-0.5"} flex min-h-[2.75rem] w-full max-w-[16rem] items-start justify-center`}>
                {showRecordError ? (
                  <div className="w-full rounded-lg border border-danger/20 bg-danger/10 px-2.5 py-1.5">
                    <div className="flex items-start gap-2 text-left">
                      <AlertCircle size={13} className="mt-0.5 shrink-0 text-danger" />
                      <div className="space-y-0.5">
                        <p className="text-[11px] font-medium leading-tight text-red-50">{transcript}</p>
                        <p className="text-[10px] leading-tight text-red-100/60">Try again or check settings.</p>
                      </div>
                    </div>
                  </div>
                ) : showRecordTranscript ? (
                  <div className="custom-scrollbar max-h-[2.75rem] overflow-y-auto px-1">
                    <p className="text-[11px] leading-tight text-slate-100">{transcript}</p>
                  </div>
                ) : showSettingsNotice ? (
                  <div className="w-full max-w-[14rem] rounded-lg border border-primary/20 bg-primary/10 px-2 py-1.5">
                    <p className="text-[10px] leading-tight text-amber-50">
                      Add API key in settings.
                    </p>
                    <button
                      type="button"
                      onClick={() => void handleOpenSettings()}
                      className={`focus-ring mt-1 rounded-full border border-primary/30 bg-black/20 px-2 py-0.5 text-[9px] font-medium text-primary ${controlMotion} hover:bg-white/10`}
                    >
                      Settings
                    </button>
                  </div>
                ) : (
                  <div className="h-[2.75rem]" aria-hidden="true" />
                )}
              </div>
            </div>
          </div>
        ) : (
          <div
            id="history-panel"
            role="tabpanel"
            aria-labelledby="history-tab"
            className="flex h-full flex-col no-drag"
          >
            <div className="custom-scrollbar flex-1 overflow-y-auto px-3 pb-3">
              {history.map((item) => (
                <div key={item.id} className={`relative -mx-1 rounded-lg px-1 py-2 ${controlMotion} hover:bg-white/5`}>
                  <p className="pr-1 text-xs leading-5 text-slate-200">{item.text}</p>
                  <div className="mt-1.5 flex items-center justify-between">
                    <span className="text-[10px] text-slate-600 font-mono">
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                    <div className="flex items-center gap-1 text-slate-500">
                      <button
                        type="button"
                        onClick={() => copyToClipboard(item.text)}
                        className={`focus-ring rounded p-1 ${controlMotion} hover:text-white`}
                        aria-label="Copy transcript"
                      >
                        <Copy size={10} />
                      </button>
                      <button
                        type="button"
                        onClick={() => repasteHistory(item.text)}
                        className={`focus-ring rounded p-1 ${controlMotion} hover:text-primary`}
                        aria-label="Re-paste transcript"
                      >
                        <RefreshCw size={10} />
                      </button>
                      <button
                        type="button"
                        onClick={() => deleteHistory(item.id)}
                        className={`focus-ring rounded p-1 ${controlMotion} hover:text-red-400`}
                        aria-label="Delete transcript"
                      >
                        <Trash2 size={10} />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
              
              {history.length === 0 && (
                <div className="flex h-full flex-col items-center justify-center pb-4 text-center text-slate-500 pointer-events-none">
                  <HistoryIcon size={20} className="mb-2 opacity-50" />
                  <p className="text-sm font-medium text-slate-200">
                    Dictate something to create your first history entry.
                  </p>
                  <p className="mt-1 max-w-[14rem] text-xs leading-5 text-slate-400">
                    Your past dictations will appear here so you can copy, re-paste, or delete them later.
                  </p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
