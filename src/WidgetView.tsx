import { useEffect, useRef, useState } from "react";
import type { MouseEventHandler, RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, X } from "lucide-react";
import { FamVoiceLockup } from "./components/FamVoiceLockup";
import { FamVoiceLogo } from "./FamVoiceLogo";
import { FamVoiceMarkLive } from "./components/FamVoiceMarkLive";
import type { Status } from "./appTypes";

const MIC_WARNING_LEVEL_THRESHOLD = 0.035;
const MIC_WARNING_INITIAL_DELAY_MS = 1200;
const MIC_WARNING_INACTIVITY_DELAY_MS = 1800;
const MIC_WARNING_POLL_INTERVAL_MS = 150;

/**
 * The mark at rest and the mark carrying the whole surface. The active size has
 * to read at a glance on its own; the resting one only has to sit beside the
 * wordmark, so it stays at lockup scale.
 */
const REST_MARK_SIZE = 22;
const ACTIVE_MARK_SIZE = 46;

interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  highlightKey?: number;
  errorMessage?: string;
  retryAvailable: boolean;
  isRetrying: boolean;
  onRetry: () => void;
  onDiscardRetry: () => void;
  containerRef: RefObject<HTMLElement | null>;
  /**
   * The window sizer. It tracks the union of every widget state, not the pill,
   * so the pill can morph between states without dragging a window resize
   * across the Tauri bridge on every animation frame.
   */
  sizeRef: RefObject<HTMLDivElement | null>;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  highlightKey,
  errorMessage,
  retryAvailable,
  isRetrying,
  onRetry,
  onDiscardRetry,
  containerRef,
  sizeRef,
  onMouseDownCapture,
}: WidgetViewProps) {
  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";
  const [isFinishing, setIsFinishing] = useState(false);
  const [showMicWarning, setShowMicWarning] = useState(false);
  const previousStatusRef = useRef<Status>(status);
  const finishTimeoutRef = useRef<number | null>(null);
  const showError = status === "error" || retryAvailable;
  const showIssue = showError || (status === "idle" && missingApiKey);
  const statusLabel = showError ? "Error" : "API key missing";
  const statusCopy = showError
    ? retryAvailable
      ? "Failed audio held briefly."
      : errorMessage === "No voice detected"
      ? "No speech found."
      : "Try again."
    : "Tray menu → Settings.";
  const statusDotClassName = showError
    ? "bg-danger shadow-[0_0_10px_rgba(179,93,79,0.32)]"
    : "bg-primary shadow-[0_0_10px_rgba(209,122,40,0.28)]";
  const statusTextClassName = showError ? "text-danger" : "text-primary";
  const isCompactWaveState = !showIssue && (status === "recording" || status === "transcribing" || isFinishing);
  const showWarningRing = showMicWarning || (status === "idle" && missingApiKey);
  const shellClassName = `${isCompactWaveState
    ? "widget-shell widget-shell--compact relative rounded-[18px] p-2 overflow-hidden"
    : "widget-shell relative rounded-[16px] pl-2 pr-1 py-1.5 overflow-hidden"}${showWarningRing ? " widget-shell--mic-warning" : ""}`;
  const rowClassName = isCompactWaveState
    ? "flex w-full items-center justify-center p-0"
    : "flex w-full items-center pl-1.5 pr-0.5 py-1";
  // The active widget is the mark and nothing else. Both marks fill the slot,
  // which is what animates: the slot grows from lockup scale to active scale
  // while the two marks cross-fade inside it, so one shape appears to wake up.
  const activeMarkClassName = "h-full w-full";
  const markSlotSize = isCompactWaveState ? ACTIVE_MARK_SIZE : REST_MARK_SIZE;
  const morphState = isCompactWaveState ? "active" : "rest";
  const waveWrapClassName = isFinishing
    ? "widget-wave-wrap widget-wave-wrap--finish"
    : "widget-wave-wrap";
  const renderedWaveMode = waveMode === "idle" && isFinishing ? "transcribing" : waveMode;
  const liveStatusMessage = status === "recording"
    ? "Recording started."
    : status === "transcribing"
      ? "Transcription in progress."
      : status === "success"
        ? "Transcript ready."
        : "";
  // The window holds one size for every state, so this mirror has to cover them
  // all at once: the lockup sets the width, the active mark sets the height, and
  // the pill morphs inside the box they agree on. The padding and border here
  // track the two .widget-shell variants above — they are the same box.
  const widgetSizeAnchor = (
    <div className="pointer-events-none invisible">
      <div className="grid">
        <div className="col-start-1 row-start-1 border pl-2 pr-1 py-1.5">
          <div className="flex items-center pl-1.5 pr-0.5 py-1">
            <FamVoiceLockup markSize={22} />
          </div>
        </div>
        <div className="col-start-1 row-start-1 border p-2">
          <div className="flex items-center p-0">
            <div
              aria-hidden="true"
              style={{ width: ACTIVE_MARK_SIZE, height: ACTIVE_MARK_SIZE }}
            />
          </div>
        </div>
      </div>
    </div>
  );

  useEffect(() => {
    if (!highlightKey || !containerRef.current) return;
    const el = containerRef.current as HTMLElement;
    el.classList.remove("widget-highlight");
    void el.offsetHeight; // force reflow to restart animation
    el.classList.add("widget-highlight");
  }, [highlightKey, containerRef]);

  useEffect(() => {
    const previousStatus = previousStatusRef.current;
    let nextFinishingState: boolean | null = null;

    if (previousStatus === "recording" && (status === "transcribing" || status === "success")) {
      nextFinishingState = true;

      if (finishTimeoutRef.current !== null) {
        window.clearTimeout(finishTimeoutRef.current);
      }

      finishTimeoutRef.current = window.setTimeout(() => {
        setIsFinishing(false);
        finishTimeoutRef.current = null;
      }, 360);
    } else if (status === "recording" || showIssue) {
      nextFinishingState = false;

      if (finishTimeoutRef.current !== null) {
        window.clearTimeout(finishTimeoutRef.current);
        finishTimeoutRef.current = null;
      }
    }

    previousStatusRef.current = status;

    if (nextFinishingState !== null) {
      queueMicrotask(() => {
        setIsFinishing((current) => (
          current === nextFinishingState ? current : nextFinishingState
        ));
      });
    }
  }, [showIssue, status]);

  useEffect(() => {
    if (status !== "recording") {
      queueMicrotask(() => {
        setShowMicWarning(false);
      });
      return;
    }

    queueMicrotask(() => {
      setShowMicWarning(false);
    });

    let lastHeardAt = Date.now();
    let hasDetectedSpeech = false;
    const startedAt = lastHeardAt;

    const syncMicWarning = () => {
      const now = Date.now();
      const shouldWarn = hasDetectedSpeech
        ? now - lastHeardAt >= MIC_WARNING_INACTIVITY_DELAY_MS
        : now - startedAt >= MIC_WARNING_INITIAL_DELAY_MS;

      setShowMicWarning((current) => (current === shouldWarn ? current : shouldWarn));
    };

    const intervalId = window.setInterval(syncMicWarning, MIC_WARNING_POLL_INTERVAL_MS);
    const unlisten = listen<number>("mic-level", (event) => {
      if (event.payload < MIC_WARNING_LEVEL_THRESHOLD) {
        return;
      }

      hasDetectedSpeech = true;
      lastHeardAt = Date.now();
      setShowMicWarning(false);
    });

    return () => {
      window.clearInterval(intervalId);
      void unlisten.then((fn) => fn());
    };
  }, [status]);

  useEffect(() => {
    return () => {
      if (finishTimeoutRef.current !== null) {
        window.clearTimeout(finishTimeoutRef.current);
      }
    };
  }, []);

  return (
    <div className="w-full h-full flex items-center justify-center p-2" style={{ pointerEvents: "none" }}>
      <div ref={sizeRef} id="widget-envelope" className="relative">
        {widgetSizeAnchor}

        <div className="absolute inset-0 flex items-center justify-center">
          <main
            ref={containerRef}
            id="widget-container"
            className={shellClassName}
            style={{ pointerEvents: "auto" }}
            data-widget-morph={morphState}
            onMouseDownCapture={onMouseDownCapture}
            onContextMenu={(e) => {
              e.preventDefault();
            }}
          >
            <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">
              {liveStatusMessage}
            </p>
            {showIssue ? (
              <div className="flex w-full items-center pl-1.5 pr-0.5 py-1">
                <div className="relative flex min-w-0 flex-1 items-center select-none">
                  <FamVoiceLockup aria-hidden="true" markSize={22} wordmarkClassName="opacity-0" />
                  <div
                    className="absolute inset-y-0 right-0 left-[28px] flex min-w-0 flex-col justify-center"
                    role={showError ? "alert" : undefined}
                    aria-atomic={showError ? true : undefined}
                  >
                    <div className="flex items-center gap-1.5">
                      <div className={`h-1 w-1 shrink-0 rounded-full ${statusDotClassName}`} />
                      <p className={`truncate text-[10px] font-bold leading-none ${statusTextClassName}`}>
                        {statusLabel}
                      </p>
                    </div>
                    {showError && retryAvailable ? (
                      <div className="mt-0.5 flex items-center gap-1">
                        <button
                          type="button"
                          onClick={onRetry}
                          disabled={isRetrying}
                          className="focus-ring inline-flex min-h-5 items-center gap-1 rounded-full border border-danger/20 bg-danger/10 px-1.5 text-[8px] font-bold text-red-50 disabled:opacity-50"
                          aria-label="Retry last dictation"
                        >
                          <RefreshCw size={8} aria-hidden="true" />
                          {isRetrying ? "Retrying" : "Retry"}
                        </button>
                        <button
                          type="button"
                          onClick={onDiscardRetry}
                          className="focus-ring flex size-5 items-center justify-center rounded-full text-slate-400 hover:text-white"
                          aria-label="Discard failed dictation audio"
                        >
                          <X size={9} aria-hidden="true" />
                        </button>
                      </div>
                    ) : (
                      <p className="truncate text-[9px] leading-tight text-slate-400">
                        {statusCopy}
                      </p>
                    )}
                  </div>
                </div>
              </div>
            ) : (
              // One lockup for every non-issue state. The wordmark collapses out
              // of it and the mark grows into the space, so idle and active are
              // two ends of one move rather than two components swapping.
              <div className={`widget-morph-row ${rowClassName}`}>
                <FamVoiceLockup
                  className="widget-status flex min-w-0 items-center justify-center pointer-events-none select-none"
                  collapsible
                  collapsed={isCompactWaveState}
                  mark={
                    <div
                      className="widget-mark-slot"
                      style={{ width: markSlotSize, height: markSlotSize }}
                    >
                      <div className="widget-mark-layer widget-mark-layer--rest">
                        <FamVoiceLogo size={REST_MARK_SIZE} className={activeMarkClassName} />
                      </div>
                      {/* Kept mounted through idle so it can cross-fade in place
                          instead of appearing the instant recording starts. */}
                      <div className="widget-mark-layer widget-mark-layer--live">
                        <div className={waveWrapClassName}>
                          <FamVoiceMarkLive mode={renderedWaveMode} className={activeMarkClassName} />
                        </div>
                      </div>
                    </div>
                  }
                />
              </div>
            )}
          </main>
        </div>
      </div>
    </div>
  );
}
