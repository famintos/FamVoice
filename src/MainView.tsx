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

  const showStatusDot = status === "success" || status === "error";
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
        ? "Transcript captured"
        : status === "error"
          ? "Attention required"
          : "Console ready";
  const statusFlag = showStatusDot
    ? status === "success"
      ? "OK"
      : "ERR"
    : status === "recording"
      ? "REC"
      : status === "transcribing"
        ? "SYNC"
        : "IDLE";
  const stageHint = status === "recording"
    ? "Release the hotkey to send this capture for transcription."
    : status === "transcribing"
      ? "Audio uploaded. Waiting for the transcription service."
      : status === "success"
        ? "The latest transcript is ready for paste-back."
        : status === "error"
          ? "Review the readout for failure details."
          : "Standing by for the global dictation hotkey.";
  const readoutText = transcript
    ? transcript
    : status === "recording"
      ? "Microphone input is live. Keep holding the hotkey until you finish speaking."
      : status === "transcribing"
        ? "Processing the most recent capture."
        : status === "success"
          ? "Transcript delivered."
          : status === "error"
            ? "The last request failed before a transcript was returned."
            : missingTranscriptionKey || missingPromptOptimizerKey
              ? "Configuration required. Open Settings to finish key setup."
              : "System idle. Hold the global hotkey to start dictation.";

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
      className="signal-shell signal-shell--main relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[28px]"
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(209,122,40,0.18),transparent_34%),linear-gradient(180deg,rgba(255,255,255,0.02),transparent_16%)]"
      />

      {pendingUpdate && isUpdateNoticeOpen && (
        <div className="absolute inset-x-4 top-4 z-20 no-drag">
          <div className="status-panel status-panel--update" style={{ borderRadius: 22, overflow: "hidden" }}>
            <div className="flex items-start justify-between gap-3 px-4 py-3">
              <div className="min-w-0 space-y-1">
                <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary/80">Updater</p>
                <p className="text-[12px] font-semibold text-slate-100">A new update is available</p>
                <p className="font-mono text-[10px] text-primary">v{pendingUpdate.version}</p>
                <p className="text-[11px] leading-5 text-slate-400">
                  Open Settings to download and install it manually.
                </p>
              </div>
              <button
                onClick={() => {
                  dismissUpdateNotice();
                }}
                className="rounded-full p-1.5 text-slate-500 transition-colors hover:bg-white/10 hover:text-white"
                aria-label="Dismiss update notice"
              >
                <X size={14} />
              </button>
            </div>
            <div className="flex justify-end px-4 pb-4">
              <button
                onClick={() => {
                  dismissUpdateNotice();
                  void handleOpenSettings();
                }}
                className="rounded-full border border-primary/25 bg-white/5 px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.2em] text-amber-100 transition-colors hover:bg-white/10"
              >
                Open Settings
              </button>
            </div>
          </div>
        </div>
      )}

      <div data-tauri-drag-region className="relative z-10 flex items-center justify-between border-b border-white/6 px-4 py-2.5">
        <div className="flex items-center gap-2 pointer-events-none select-none text-slate-300">
          <FamVoiceLogo size={14} />
          <div className="flex flex-col">
            <span className="font-mono text-[10px] uppercase tracking-[0.24em] text-slate-500">Signal Console</span>
            <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-200">FamVoice</span>
          </div>
        </div>

        <div className="flex items-center gap-1.5 no-drag">
          <button
            onClick={handleOpenSettings}
            className="rounded-full p-1.5 text-slate-500 transition-colors hover:bg-white/10 hover:text-white"
            aria-label="Open settings"
          >
            <SettingsIcon size={14} />
          </button>
          <button
            onClick={() => appWindow.minimize()}
            className="rounded-full p-1.5 text-slate-500 transition-colors hover:bg-white/10 hover:text-white"
            aria-label="Minimize window"
          >
            <Minus size={14} />
          </button>
          <button
            onClick={() => appWindow.close()}
            className="rounded-full p-1.5 text-slate-500 transition-colors hover:bg-white/10 hover:text-red-300"
            aria-label="Close window"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      <div className="relative z-10 border-b border-white/6 px-4 py-2 no-drag">
        <div className="inline-flex rounded-full border border-white/8 bg-black/10 p-1 select-none">
          <button
            onClick={() => setActiveTab("record")}
            className={`rounded-full px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.22em] transition-colors cursor-pointer ${activeTab === "record" ? "bg-white/10 text-primary" : "text-slate-500 hover:text-slate-200"
              }`}
          >
            Dictate
          </button>
          <button
            onClick={() => setActiveTab("history")}
            className={`rounded-full px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.22em] transition-colors cursor-pointer ${activeTab === "history" ? "bg-white/10 text-primary" : "text-slate-500 hover:text-slate-200"
              }`}
          >
            History
          </button>
        </div>
      </div>

      <div className="relative z-10 flex-1 overflow-hidden">
        {activeTab === "record" ? (
          <div className="flex h-full min-h-0 flex-col px-3 py-3">
            <div className="custom-scrollbar no-drag flex flex-1 flex-col gap-2 overflow-y-auto pr-1">
              <section className="signal-stage flex flex-col gap-3 rounded-[24px] px-4 py-3.5">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="font-mono text-[10px] uppercase tracking-[0.2em] text-slate-500">Active stage</p>
                    <h2 className="mt-1 text-base font-semibold tracking-[0.02em] text-slate-100">{statusLabel}</h2>
                  </div>
                  <div className="rounded-full border border-white/10 bg-black/15 px-2.5 py-1 font-mono text-[10px] uppercase tracking-[0.2em] text-slate-400">
                    {statusFlag}
                  </div>
                </div>

                <div className="flex items-center justify-center py-1">
                  <div className="rounded-[20px] border border-white/6 bg-black/15 px-6 py-4 shadow-[0_14px_28px_rgba(0,0,0,0.24)]">
                    <VoiceWave mode={waveMode} size="large" />
                  </div>
                </div>

                <p className="pointer-events-none text-center text-[11px] leading-4 text-slate-400">
                  {stageHint}
                </p>
              </section>

              <section className="signal-readout shrink-0 rounded-[18px]">
                <div className="flex items-start gap-2.5 px-3.5 py-2.5">
                  <span className="pt-0.5 font-mono text-[10px] uppercase tracking-[0.2em] text-slate-500">
                    Readout
                  </span>
                  <div className="min-w-0 flex-1 text-[11px] leading-4">
                    {status === "error" && transcript ? (
                      <div className="flex items-start gap-2 text-rose-300">
                        <AlertCircle size={14} className="mt-0.5 shrink-0" />
                        <span>{transcript}</span>
                      </div>
                    ) : (
                      <p className={transcript ? "text-slate-100" : "text-slate-400"}>{readoutText}</p>
                    )}
                  </div>
                </div>
              </section>

              {status === "idle" && !transcript && (missingTranscriptionKey || missingPromptOptimizerKey) && (
                <div className="flex shrink-0 flex-col gap-2">
                  {missingTranscriptionKey && (
                    <div className="status-panel status-panel--warning" style={{ borderRadius: 18, overflow: "hidden" }}>
                      <button
                        onClick={handleOpenSettings}
                        className="flex w-full items-center justify-between gap-3 px-3.5 py-2.5 text-left cursor-pointer"
                      >
                        <span>
                          <span className="block font-mono text-[10px] uppercase tracking-[0.2em] text-amber-200/70">
                            Warning
                          </span>
                          <span className="mt-1 block text-[11px] font-semibold text-amber-100">
                            {settings.transcription_provider === "groq" ? "Groq" : "OpenAI"} key missing
                          </span>
                        </span>
                        <span className="font-mono text-[10px] uppercase tracking-[0.16em] text-amber-200/80">
                          Settings
                        </span>
                      </button>
                    </div>
                  )}
                  {missingPromptOptimizerKey && (
                    <div className="status-panel status-panel--warning" style={{ borderRadius: 18, overflow: "hidden" }}>
                      <button
                        onClick={handleOpenSettings}
                        className="flex w-full items-center justify-between gap-3 px-3.5 py-2.5 text-left cursor-pointer"
                      >
                        <span>
                          <span className="block font-mono text-[10px] uppercase tracking-[0.2em] text-amber-200/70">
                            Warning
                          </span>
                          <span className="mt-1 block text-[11px] font-semibold text-amber-100">
                            Prompt optimization OpenAI key missing
                          </span>
                        </span>
                        <span className="font-mono text-[10px] uppercase tracking-[0.16em] text-amber-200/80">
                          Settings
                        </span>
                      </button>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="flex h-full min-h-0 flex-col no-drag">
            <div className="flex items-center justify-between border-b border-white/6 px-4 py-3">
              <div className="pointer-events-none">
                <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-slate-500">Utility log</p>
                <p className="text-[12px] text-slate-300">{history.length} locally cached entries</p>
              </div>
              {history.length > 0 && (
                <button
                  onClick={clearHistory}
                  className="flex items-center gap-1.5 rounded-full border border-white/8 bg-white/5 px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.18em] text-slate-400 transition-colors cursor-pointer hover:text-red-300"
                >
                  <Trash2 size={12} /> Clear
                </button>
              )}
            </div>
            <div className="custom-scrollbar flex-1 overflow-y-auto px-3 py-3">
              {history.map((item) => (
                <article key={item.id} className="utility-log-row rounded-[16px] px-2.5 py-2.5">
                  <div className="flex items-start justify-between gap-3">
                    <p className="min-w-0 flex-1 line-clamp-2 text-[11px] leading-4 text-slate-100">{item.text}</p>
                    <span className="shrink-0 font-mono text-[10px] uppercase tracking-[0.18em] text-slate-500">
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    <button
                      onClick={() => copyToClipboard(item.text)}
                      className="flex items-center gap-1 rounded-full border border-white/8 bg-white/5 px-2 py-1 font-mono text-[10px] uppercase tracking-[0.14em] text-slate-300 transition-colors cursor-pointer hover:text-primary"
                      title="Copy"
                    >
                      <Copy size={11} />
                      Copy
                    </button>
                    <button
                      onClick={() => repasteHistory(item.text)}
                      className="flex items-center gap-1 rounded-full border border-white/8 bg-white/5 px-2 py-1 font-mono text-[10px] uppercase tracking-[0.14em] text-slate-300 transition-colors cursor-pointer hover:text-green-300"
                      title="Re-paste"
                    >
                      <RefreshCw size={11} />
                      Re-paste
                    </button>
                    <button
                      onClick={() => deleteHistory(item.id)}
                      className="flex items-center gap-1 rounded-full border border-white/8 bg-white/5 px-2 py-1 font-mono text-[10px] uppercase tracking-[0.14em] text-slate-300 transition-colors cursor-pointer hover:text-red-300"
                      title="Delete"
                    >
                      <Trash2 size={11} />
                      Delete
                    </button>
                  </div>
                </article>
              ))}
              {history.length === 0 && (
                <div className="flex h-full flex-col items-center justify-center py-12 text-slate-500 pointer-events-none">
                  <HistoryIcon size={30} className="mb-3" />
                  <p className="font-mono text-[10px] uppercase tracking-[0.22em]">No history yet</p>
                  <p className="mt-2 text-[11px] text-slate-600">Transcripts will appear here after a successful paste-back.</p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
