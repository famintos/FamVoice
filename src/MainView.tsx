import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  AlertCircle,
  CheckCircle2,
  Copy,
  History as HistoryIcon,
  Minus,
  Info,
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
const HISTORY_TIMESTAMP_FORMATTER = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
  timeStyle: "short",
});

const TOAST_AUTO_DISMISS_MS = 2800;

type ToastVariant = "success" | "error" | "neutral";

interface ToastEntry {
  id: number;
  title: string;
  description?: string;
  variant: ToastVariant;
}

function formatHistoryTimestamp(timestamp: number): string {
  return HISTORY_TIMESTAMP_FORMATTER.format(new Date(timestamp));
}

function ToastIcon({ variant }: { variant: ToastVariant }) {
  if (variant === "success") {
    return <CheckCircle2 size={14} className="shrink-0 text-green-400" />;
  }

  if (variant === "error") {
    return <AlertCircle size={14} className="shrink-0 text-red-400" />;
  }

  return <Info size={14} className="shrink-0 text-slate-300" />;
}

function ToastStack({
  toasts,
  onDismiss,
}: {
  toasts: ToastEntry[];
  onDismiss: (id: number) => void;
}) {
  if (toasts.length === 0) {
    return null;
  }

  return (
    <div className="absolute inset-x-3 top-10 z-30 flex flex-col gap-2 no-drag pointer-events-none">
      {toasts.map((toast) => {
        const toneClassName = toast.variant === "success"
          ? "border-green-500/20 bg-green-500/10 text-green-50"
          : toast.variant === "error"
            ? "border-red-500/20 bg-red-500/10 text-red-50"
            : "border-white/10 bg-black/45 text-slate-100";

        return (
          <div
            key={toast.id}
            className={`pointer-events-auto rounded-xl border px-3 py-2 shadow-[0_18px_40px_rgba(0,0,0,0.35)] backdrop-blur-sm ${toneClassName}`}
          >
            <div className="flex items-start gap-2">
              <ToastIcon variant={toast.variant} />
              <div className="min-w-0 flex-1">
                <p className="text-xs font-semibold leading-tight">
                  {toast.title}
                </p>
                {toast.description ? (
                  <p className="mt-1 text-[11px] leading-snug text-white/70">
                    {toast.description}
                  </p>
                ) : null}
              </div>
              <button
                type="button"
                onClick={() => onDismiss(toast.id)}
                className="focus-ring -mr-1 rounded p-0.5 text-white/40 transition-colors hover:text-white"
                aria-label="Dismiss notification"
              >
                <X size={11} />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function ClearHistoryDialog({
  open,
  count,
  isSubmitting,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  count: number;
  isSubmitting: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  if (!open) {
    return null;
  }

  return (
    <div
      className="absolute inset-0 z-40 flex items-center justify-center bg-slate-950/70 px-4 py-4 backdrop-blur-sm no-drag"
      role="presentation"
      onMouseDown={onCancel}
    >
      <div
        className="w-full max-w-[18rem] rounded-2xl border border-white/10 bg-[#111723] p-4 text-left shadow-[0_22px_60px_rgba(0,0,0,0.45)]"
        role="dialog"
        aria-modal="true"
        aria-labelledby="clear-history-dialog-title"
        aria-describedby="clear-history-dialog-description"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-3">
          <div className="mt-0.5 rounded-full border border-red-500/20 bg-red-500/10 p-2 text-red-400">
            <Trash2 size={14} />
          </div>
          <div className="min-w-0 flex-1">
            <h3 id="clear-history-dialog-title" className="text-sm font-semibold text-white">
              Clear history?
            </h3>
            <p id="clear-history-dialog-description" className="mt-1 text-xs leading-5 text-slate-400">
              This will delete {count} {count === 1 ? "entry" : "entries"} from your local history. This cannot be undone.
            </p>
          </div>
        </div>
        <div className="mt-4 flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="focus-ring rounded-full border border-white/10 bg-black/20 px-3 py-1.5 text-xs font-medium text-slate-300 transition-colors hover:border-white/20 hover:text-white"
            disabled={isSubmitting}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="focus-ring rounded-full border border-red-500/20 bg-red-500/15 px-3 py-1.5 text-xs font-semibold text-red-50 transition-colors hover:bg-red-500/25 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={isSubmitting}
          >
            {isSubmitting ? "Clearing..." : "Clear history"}
          </button>
        </div>
      </div>
    </div>
  );
}

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
  const [toasts, setToasts] = useState<ToastEntry[]>([]);
  const [isClearHistoryOpen, setIsClearHistoryOpen] = useState(false);
  const [isClearingHistory, setIsClearingHistory] = useState(false);
  const widgetContainerRef = useRef<HTMLElement | null>(null);
  const lastWidgetSizeRef = useRef<{ width: number; height: number } | null>(null);
  const ignoreCursorEventsRef = useRef<boolean | null>(null);
  const widgetDragGraceUntilRef = useRef(0);
  const widgetWindowMetricsRef = useRef<WidgetWindowMetrics | null>(null);
  const lastCursorPositionRef = useRef<{ x: number; y: number } | null>(null);
  const hasDismissedUpdateNoticeRef = useRef(false);
  const toastIdRef = useRef(0);
  const toastTimeoutsRef = useRef<number[]>([]);

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

  useEffect(() => {
    if (!isClearHistoryOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !isClearingHistory) {
        event.preventDefault();
        setIsClearHistoryOpen(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isClearHistoryOpen, isClearingHistory]);

  useEffect(() => {
    return () => {
      toastTimeoutsRef.current.forEach((timeoutId) => window.clearTimeout(timeoutId));
      toastTimeoutsRef.current = [];
    };
  }, []);

  const loadHistory = async () => {
    const items = await invoke<HistoryItem[]>("get_history");
    setHistory(items);
  };

  const dismissToast = (id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  };

  const showToast = (
    variant: ToastVariant,
    title: string,
    description?: string,
  ) => {
    const id = toastIdRef.current + 1;
    toastIdRef.current = id;
    setToasts((current) => [...current, { id, variant, title, description }]);

    const timeoutId = window.setTimeout(() => {
      setToasts((current) => current.filter((toast) => toast.id !== id));
      toastTimeoutsRef.current = toastTimeoutsRef.current.filter((currentId) => currentId !== timeoutId);
    }, TOAST_AUTO_DISMISS_MS);
    toastTimeoutsRef.current.push(timeoutId);
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      showToast("success", "Copied transcript", "The selected history item is now on your clipboard.");
    } catch (error) {
      console.error("Failed to copy transcript:", error);
      showToast("error", "Could not copy transcript", String(error));
    }
  };

  const repasteHistory = async (text: string) => {
    try {
      await invoke("repaste_history_item", { text });
      showToast("success", "Re-pasted transcript", "The transcript was pasted into the active app.");
    } catch (error) {
      console.error("Failed to re-paste history item:", error);
      showToast("error", "Could not re-paste transcript", String(error));
    }
  };

  const openClearHistoryConfirm = () => {
    setIsClearHistoryOpen(true);
  };

  const closeClearHistoryConfirm = () => {
    if (isClearingHistory) return;
    setIsClearHistoryOpen(false);
  };

  const confirmClearHistory = async () => {
    if (isClearingHistory) return;

    try {
      setIsClearingHistory(true);
      await invoke("clear_history");
      setIsClearHistoryOpen(false);
      showToast("success", "History cleared", "Your transcript history has been removed.");
    } catch (error) {
      console.error("Failed to clear history:", error);
      showToast("error", "Could not clear history", String(error));
    } finally {
      setIsClearingHistory(false);
    }
  };

  const dismissUpdateNotice = () => {
    hasDismissedUpdateNoticeRef.current = true;
    setIsUpdateNoticeOpen(false);
  };

  const handleOpenSettings = async () => {
    await invoke("open_settings_window");
  };

  const deleteHistory = async (id: number) => {
    await invoke("delete_history_item", { id });
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
        ? settings?.auto_paste
          ? "Pasted to your app."
          : "Ready for paste-back."
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
      <ToastStack
        toasts={toasts}
        onDismiss={dismissToast}
      />

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
                onClick={openClearHistoryConfirm}
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
                      {formatHistoryTimestamp(item.timestamp)}
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
      <ClearHistoryDialog
        open={isClearHistoryOpen}
        count={history.length}
        isSubmitting={isClearingHistory}
        onCancel={closeClearHistoryConfirm}
        onConfirm={() => void confirmClearHistory()}
      />
    </main>
  );
}
