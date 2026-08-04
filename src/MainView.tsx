import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  AlertCircle,
  CheckCircle2,
  Copy,
  Download,
  History as HistoryIcon,
  Minus,
  Info,
  Pin,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  Trash2,
  X,
} from "lucide-react";
import { WIDGET_CURSOR_POLL_INTERVAL_MS, WIDGET_DRAG_START_GRACE_MS } from "./appConstants";
import type {
  HistoryItem,
  RetryAudioStatus,
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
  actionLabel?: string;
  onAction?: () => void;
}

interface ToastOptions {
  actionLabel?: string;
  durationMs?: number;
  onAction?: () => void;
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
            role={toast.variant === "error" ? "alert" : "status"}
            aria-atomic="true"
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
                {toast.actionLabel && toast.onAction ? (
                  <button
                    type="button"
                    onClick={() => {
                      toast.onAction?.();
                      onDismiss(toast.id);
                    }}
                    className="focus-ring mt-2 min-h-6 rounded-full border border-white/15 px-2.5 text-[11px] font-semibold text-white transition-colors hover:border-primary/50 hover:text-primary"
                  >
                    {toast.actionLabel}
                  </button>
                ) : null}
              </div>
              <button
                type="button"
                onClick={() => onDismiss(toast.id)}
                className="focus-ring -mr-1 flex size-6 shrink-0 items-center justify-center rounded text-white/40 transition-colors hover:text-white"
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
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const onCancelRef = useRef(onCancel);
  const isSubmittingRef = useRef(isSubmitting);

  useEffect(() => {
    onCancelRef.current = onCancel;
    isSubmittingRef.current = isSubmitting;
  }, [isSubmitting, onCancel]);

  useEffect(() => {
    if (!open) return;

    cancelButtonRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (!isSubmittingRef.current) {
          event.preventDefault();
          onCancelRef.current();
        }
        return;
      }

      if (event.key !== "Tab") return;

      const dialog = dialogRef.current;
      if (!dialog) return;

      const focusableElements = Array.from(dialog.querySelectorAll<HTMLElement>(
        "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
      ));

      if (focusableElements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];
      const activeElement = document.activeElement;

      if (event.shiftKey && (activeElement === firstElement || !dialog.contains(activeElement))) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && (activeElement === lastElement || !dialog.contains(activeElement))) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

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
        ref={dialogRef}
        className="w-full max-w-[18rem] rounded-2xl border border-white/10 bg-[#111723] p-4 text-left shadow-[0_22px_60px_rgba(0,0,0,0.45)]"
        role="dialog"
        aria-modal="true"
        aria-busy={isSubmitting}
        aria-labelledby="clear-history-dialog-title"
        aria-describedby="clear-history-dialog-description"
        tabIndex={-1}
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
              This permanently deletes {count} {count === 1 ? "entry" : "entries"} and FamVoice recovery copies. Exported files are unaffected. This cannot be undone.
            </p>
          </div>
        </div>
        <div className="mt-4 flex items-center justify-end gap-2">
          <button
            ref={cancelButtonRef}
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
  const [historyQuery, setHistoryQuery] = useState("");
  const [retryAudio, setRetryAudio] = useState<RetryAudioStatus>({ available: false });
  const [isRetrying, setIsRetrying] = useState(false);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [isUpdateNoticeOpen, setIsUpdateNoticeOpen] = useState(false);
  const [highlightKey, setHighlightKey] = useState(0);
  const [toasts, setToasts] = useState<ToastEntry[]>([]);
  const [isClearHistoryOpen, setIsClearHistoryOpen] = useState(false);
  const [isClearingHistory, setIsClearingHistory] = useState(false);
  const recordTabRef = useRef<HTMLButtonElement>(null);
  const historyTabRef = useRef<HTMLButtonElement>(null);
  const clearHistoryButtonRef = useRef<HTMLButtonElement>(null);
  const clearHistoryWasOpenRef = useRef(false);
  const widgetContainerRef = useRef<HTMLElement | null>(null);
  const widgetSizeRef = useRef<HTMLDivElement | null>(null);
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
    invoke<HistoryItem[]>("get_history").then(setHistory);
    invoke<RetryAudioStatus>("get_retry_audio_state").then(setRetryAudio);

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

    const unlistenRetryAudio = listen<RetryAudioStatus>("retry-audio-state", (event) => {
      setRetryAudio(event.payload);
    });

    const unlistenHighlight = listen("highlight-widget", () => {
      setHighlightKey((k) => k + 1);
    });

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenTranscript.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
      unlistenRetryAudio.then((fn) => fn());
      unlistenHighlight.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!retryAudio.available) return;
    const interval = window.setInterval(() => {
      void invoke<RetryAudioStatus>("get_retry_audio_state")
        .then((nextStatus) => {
          if (!nextStatus.available) {
            setTranscript("");
          }
          setRetryAudio(nextStatus);
        })
        .catch(() => setRetryAudio({ available: false }));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [retryAudio.available]);

  useEffect(() => {
    if (!settings?.widget_mode) {
      lastWidgetSizeRef.current = null;
      return;
    }

    // Sized off the envelope, not the pill: the envelope spans every widget
    // state at once, so the pill can animate between states without a window
    // resize on each frame of the morph.
    const sizeElement = widgetSizeRef.current;
    if (!sizeElement) return;

    let frameId = 0;
    const resizeWindow = async () => {
      const size = getWidgetWindowSizeWithChrome(sizeElement.getBoundingClientRect());
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

    observer.observe(sizeElement);
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
    let cursorSyncInFlight = false;
    let cursorSyncQueued = false;

    const loadWindowMetrics = async (): Promise<WidgetWindowMetrics> => {
      const [windowPosition, scaleFactor] = await Promise.all([
        appWindow.innerPosition(),
        appWindow.scaleFactor(),
      ]);

      return { windowPosition, scaleFactor };
    };

    const performCursorInteractivitySync = async () => {
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

    const syncCursorInteractivity = async () => {
      if (cancelled) return;

      if (cursorSyncInFlight) {
        cursorSyncQueued = true;
        return;
      }

      cursorSyncInFlight = true;
      try {
        do {
          cursorSyncQueued = false;
          await performCursorInteractivitySync();
        } while (cursorSyncQueued && !cancelled);
      } finally {
        cursorSyncInFlight = false;
      }
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
    // Pointer enter/leave cannot wake a transparent click-through WebView after
    // setIgnoreCursorEvents(true), so polling remains the recovery mechanism.
    // The single-flight queue above keeps the 75 ms checks from overlapping.
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
    if (isClearHistoryOpen) {
      clearHistoryWasOpenRef.current = true;
      return;
    }

    if (!clearHistoryWasOpenRef.current) return;
    clearHistoryWasOpenRef.current = false;

    const clearHistoryButton = clearHistoryButtonRef.current;
    if (clearHistoryButton?.isConnected) {
      clearHistoryButton.focus();
      return;
    }

    historyTabRef.current?.focus();
  }, [isClearHistoryOpen]);

  useEffect(() => {
    return () => {
      toastTimeoutsRef.current.forEach((timeoutId) => window.clearTimeout(timeoutId));
      toastTimeoutsRef.current = [];
    };
  }, []);

  const dismissToast = (id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  };

  const showToast = (
    variant: ToastVariant,
    title: string,
    description?: string,
    options: ToastOptions = {},
  ) => {
    const id = toastIdRef.current + 1;
    toastIdRef.current = id;
    setToasts((current) => [
      ...current,
      {
        id,
        variant,
        title,
        description,
        actionLabel: options.actionLabel,
        onAction: options.onAction,
      },
    ]);

    const timeoutId = window.setTimeout(() => {
      setToasts((current) => current.filter((toast) => toast.id !== id));
      toastTimeoutsRef.current = toastTimeoutsRef.current.filter((currentId) => currentId !== timeoutId);
    }, options.durationMs ?? TOAST_AUTO_DISMISS_MS);
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

  const restoreHistory = async (item: HistoryItem) => {
    try {
      await invoke("restore_history_item", { item });
      showToast("success", "Transcript restored", "The history entry is back in its original position.");
    } catch (error) {
      console.error("Failed to restore history item:", error);
      showToast("error", "Could not restore transcript", String(error));
    }
  };

  const deleteHistory = async (id: number) => {
    try {
      const deletedItem = await invoke<HistoryItem>("delete_history_item", { id });
      showToast("neutral", "Transcript deleted", undefined, {
        actionLabel: "Undo",
        durationMs: 6000,
        onAction: () => void restoreHistory(deletedItem),
      });
    } catch (error) {
      console.error("Failed to delete history item:", error);
      showToast("error", "Could not delete transcript", String(error));
    }
  };

  const toggleHistoryPin = async (id: number) => {
    try {
      await invoke("toggle_history_pin", { id });
    } catch (error) {
      showToast("error", "Could not update pin", String(error));
    }
  };

  const exportHistory = async (format: "txt" | "markdown" | "json") => {
    try {
      const path = await invoke<string>("export_history", { format });
      showToast("success", "History exported", path);
    } catch (error) {
      showToast("error", "Could not export history", String(error));
    }
  };

  const retryLastDictation = async () => {
    if (isRetrying) return;
    setIsRetrying(true);
    try {
      await invoke("retry_last_dictation");
    } catch (error) {
      console.error("Failed to retry dictation:", error);
    } finally {
      setIsRetrying(false);
    }
  };

  const discardRetryAudio = async () => {
    try {
      await invoke("discard_last_failed_dictation");
      setRetryAudio({ available: false });
      setTranscript("");
      setStatus("idle");
    } catch (error) {
      showToast("error", "Could not discard failed dictation", String(error));
    }
  };

  const handleTabKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;

    event.preventDefault();
    const nextTab = event.key === "ArrowLeft" || event.key === "Home" ? "record" : "history";
    setActiveTab(nextTab);
    window.requestAnimationFrame(() => {
      (nextTab === "record" ? recordTabRef.current : historyTabRef.current)?.focus();
    });
  };

  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";

  const missingTranscriptionKey = settings && (
    (settings.transcription_provider === "groq" && !settings.groq_api_key_present) ||
    (settings.transcription_provider === "openai" && !settings.api_key_present)
  );

  const missingPromptOptimizerKey = settings && settings.prompt_optimization_enabled && !settings.api_key_present;
  const showSettingsNotice = status === "idle" && !transcript && (missingTranscriptionKey || missingPromptOptimizerKey);
  const showRecordError = (status === "error" || retryAudio.available) && Boolean(transcript);
  const showRecordTranscript = !showRecordError && Boolean(transcript);
  const visibleHistory = useMemo(() => {
    const query = historyQuery.trim().toLocaleLowerCase();
    return history
      .filter((item) => !query || item.text.toLocaleLowerCase().includes(query))
      .map((item, index) => ({ item, index }))
      .sort((left, right) => Number(right.item.pinned) - Number(left.item.pinned) || left.index - right.index)
      .map(({ item }) => item);
  }, [history, historyQuery]);
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
  const showStageHint = status !== "error" && (status !== "idle" || !transcript);
  const liveStatusMessage = status === "recording"
    ? "Recording started. Release the hotkey to send."
    : status === "transcribing"
      ? "Recording stopped. Transcription in progress."
      : status === "success"
        ? settings?.auto_paste
          ? "Transcript ready. Pasted to your app."
          : "Transcript ready. Ready for paste-back."
        : "";

  if (settings?.widget_mode) {
    return (
      <WidgetView
        status={status}
        missingApiKey={!!missingTranscriptionKey}
        highlightKey={highlightKey}
        errorMessage={status === "error" ? transcript : undefined}
        retryAvailable={retryAudio.available}
        isRetrying={isRetrying}
        onRetry={() => void retryLastDictation()}
        onDiscardRetry={() => void discardRetryAudio()}
        containerRef={widgetContainerRef}
        sizeRef={widgetSizeRef}
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
      className="signal-shell relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[16px] bg-[#0F0F0F]"
    >
      <div
        className="contents"
        inert={isClearHistoryOpen ? true : undefined}
        aria-hidden={isClearHistoryOpen ? true : undefined}
      >
        <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">
          {liveStatusMessage}
        </p>
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
            className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} hover:text-white`}
            aria-label="Minimize window"
          >
            <Minus size={10} />
          </button>
          <button
            type="button"
            onClick={() => appWindow.hide()}
            className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} hover:text-red-400`}
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
              ref={recordTabRef}
              type="button"
              id="record-tab"
              role="tab"
              onClick={() => setActiveTab("record")}
              onKeyDown={handleTabKeyDown}
              aria-controls="record-panel"
              aria-selected={activeTab === "record"}
              tabIndex={activeTab === "record" ? 0 : -1}
              className={`focus-ring min-h-6 rounded-full px-2 py-1 text-[11px] font-medium tracking-tight ${controlMotion} ${
                activeTab === "record"
                  ? "bg-white/10 text-white"
                  : "text-slate-400 hover:text-slate-200"
              }`}
            >
              Record
            </button>
            <button
              ref={historyTabRef}
              type="button"
              id="history-tab"
              role="tab"
              onClick={() => setActiveTab("history")}
              onKeyDown={handleTabKeyDown}
              aria-controls="history-panel"
              aria-selected={activeTab === "history"}
              tabIndex={activeTab === "history" ? 0 : -1}
              className={`focus-ring min-h-6 rounded-full px-2 py-1 text-[11px] font-medium tracking-tight ${controlMotion} ${
                activeTab === "history"
                  ? "bg-white/10 text-white"
                  : "text-slate-400 hover:text-slate-200"
              }`}
            >
              History
            </button>
          </div>

          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => void handleOpenSettings()}
              className={`focus-ring inline-flex min-h-6 items-center gap-1 rounded-full border border-white/10 bg-black/20 px-2 py-1 text-[11px] font-medium text-slate-400 ${controlMotion} hover:border-primary/40 hover:text-white`}
              aria-label="Open settings"
            >
              <SettingsIcon size={10} />
              Settings
            </button>
            {activeTab === "history" && history.length > 0 && (
              <button
                ref={clearHistoryButtonRef}
                type="button"
                onClick={openClearHistoryConfirm}
                className={`focus-ring min-h-6 rounded-full px-2 py-1 text-[11px] font-medium tracking-tight text-slate-400 ${controlMotion} hover:text-red-400`}
                aria-haspopup="dialog"
                aria-expanded={isClearHistoryOpen}
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
                      showStageHint ? "mt-0.5 min-h-[1.5rem]" : "h-0 overflow-hidden invisible"
                    }`}
                    aria-hidden={!showStageHint}
                  >
                    {stageHint}
                  </p>
                </div>
              </div>

              <div className={`${(status === "error" || status === "success" || status === "transcribing" || Boolean(transcript)) ? "mt-1.5" : "mt-0.5"} flex min-h-[2.75rem] w-full max-w-[16rem] items-start justify-center`}>
                {showRecordError ? (
                  <div
                    className="w-full rounded-lg border border-danger/20 bg-danger/10 px-2.5 py-1.5"
                    role="alert"
                    aria-atomic="true"
                  >
                    <div className="flex items-start gap-2 text-left">
                      <AlertCircle size={13} className="mt-0.5 shrink-0 text-danger" />
                      <div className="space-y-0.5">
                        <p className="text-[11px] font-medium leading-tight text-red-50">{transcript}</p>
                        {retryAudio.available ? (
                          <div className="flex flex-wrap items-center gap-1.5 pt-1">
                            <button
                              type="button"
                              onClick={() => void retryLastDictation()}
                              disabled={isRetrying}
                              className={`focus-ring min-h-6 rounded-full border border-red-100/20 bg-black/20 px-2 text-[10px] font-semibold text-red-50 ${controlMotion} hover:border-red-100/40 disabled:opacity-50`}
                            >
                              {isRetrying ? "Retrying…" : "Retry last dictation"}
                            </button>
                            <button
                              type="button"
                              onClick={() => void discardRetryAudio()}
                              className={`focus-ring min-h-6 rounded-full px-2 text-[10px] text-red-100/60 ${controlMotion} hover:text-red-50`}
                            >
                              Discard audio
                            </button>
                          </div>
                        ) : (
                          <p className="text-[10px] leading-tight text-red-100/60">Try again or check settings.</p>
                        )}
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
            <div className="border-b border-white/[0.06] px-3 pb-2 pt-1">
              <div className="flex items-center gap-2">
                <label className="relative min-w-0 flex-1">
                  <span className="sr-only">Search history</span>
                  <Search size={11} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" aria-hidden="true" />
                  <input
                    type="search"
                    value={historyQuery}
                    onChange={(event) => setHistoryQuery(event.target.value)}
                    placeholder="Search history"
                    className="focus-ring min-h-7 w-full rounded-full border border-white/10 bg-black/20 pl-6 pr-2 text-[11px] text-white placeholder:text-slate-500"
                  />
                </label>
                <div className="flex items-center gap-0.5" aria-label="Export history">
                  {(["txt", "markdown", "json"] as const).map((format) => (
                    <button
                      key={format}
                      type="button"
                      onClick={() => void exportHistory(format)}
                      className={`focus-ring min-h-7 rounded px-1.5 text-[9px] font-semibold uppercase text-slate-400 ${controlMotion} hover:text-primary`}
                      aria-label={`Export history as ${format === "markdown" ? "Markdown" : format.toUpperCase()}`}
                    >
                      {format === "markdown" ? "MD" : format}
                    </button>
                  ))}
                  <Download size={10} className="ml-0.5 text-slate-600" aria-hidden="true" />
                </div>
              </div>
              {historyQuery ? (
                <p className="mt-1 px-1 text-[10px] text-slate-500" role="status">
                  {visibleHistory.length} {visibleHistory.length === 1 ? "match" : "matches"}
                </p>
              ) : null}
            </div>
            <div className="custom-scrollbar flex-1 overflow-y-auto px-3 pb-3">
              {visibleHistory.map((item) => (
                <div key={item.id} className={`relative -mx-1 rounded-lg px-1 py-2 ${controlMotion} hover:bg-white/5`}>
                  <p className="pr-1 text-xs leading-5 text-slate-200">{item.text}</p>
                  <div className="mt-1.5 flex items-center justify-between">
                    <span className="text-[10px] text-slate-400 font-mono">
                      {formatHistoryTimestamp(item.timestamp)}
                    </span>
                    <div className="flex items-center gap-1 text-slate-500">
                      <button
                        type="button"
                        onClick={() => void toggleHistoryPin(item.id)}
                        className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} ${item.pinned ? "text-primary" : "hover:text-primary"}`}
                        aria-label={item.pinned ? "Unpin transcript" : "Pin transcript"}
                        aria-pressed={item.pinned}
                      >
                        <Pin size={10} fill={item.pinned ? "currentColor" : "none"} />
                      </button>
                      <button
                        type="button"
                        onClick={() => copyToClipboard(item.text)}
                        className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} hover:text-white`}
                        aria-label="Copy transcript"
                      >
                        <Copy size={10} />
                      </button>
                      <button
                        type="button"
                        onClick={() => repasteHistory(item.text)}
                        className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} hover:text-primary`}
                        aria-label="Re-paste transcript"
                      >
                        <RefreshCw size={10} />
                      </button>
                      <button
                        type="button"
                        onClick={() => deleteHistory(item.id)}
                        className={`focus-ring flex size-6 items-center justify-center rounded ${controlMotion} hover:text-red-400`}
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
              {history.length > 0 && visibleHistory.length === 0 && (
                <div className="flex h-full flex-col items-center justify-center pb-4 text-center text-slate-500">
                  <Search size={20} className="mb-2 opacity-50" />
                  <p className="text-sm font-medium text-slate-200">No matching transcripts.</p>
                  <p className="mt-1 text-xs text-slate-400">Try a different search.</p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
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
