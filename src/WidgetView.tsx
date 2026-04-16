import { useEffect, useRef, useState } from "react";
import type { MouseEventHandler, RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { FamVoiceLogo } from "./FamVoiceLogo";
import { FamVoiceLockup } from "./components/FamVoiceLockup";
import { VoiceWave } from "./components/VoiceWave";
import type { Status } from "./appTypes";

const MIC_WARNING_LEVEL_THRESHOLD = 0.035;
const MIC_WARNING_INITIAL_DELAY_MS = 1200;
const MIC_WARNING_INACTIVITY_DELAY_MS = 1800;
const MIC_WARNING_POLL_INTERVAL_MS = 150;

interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  highlightKey?: number;
  errorMessage?: string;
  containerRef: RefObject<HTMLElement | null>;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  highlightKey,
  errorMessage,
  containerRef,
  onMouseDownCapture,
}: WidgetViewProps) {
  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";
  const [isFinishing, setIsFinishing] = useState(false);
  const [showMicWarning, setShowMicWarning] = useState(false);
  const previousStatusRef = useRef<Status>(status);
  const finishTimeoutRef = useRef<number | null>(null);
  const showError = status === "error";
  const showIssue = showError || (status === "idle" && missingApiKey);
  const statusLabel = showError ? "Error" : "API key missing";
  const statusCopy = showError
    ? errorMessage === "No voice detected"
      ? "No speech found."
      : "Try again."
    : "Open Settings.";
  const statusDotClassName = showError
    ? "bg-danger shadow-[0_0_10px_rgba(179,93,79,0.32)]"
    : "bg-primary shadow-[0_0_10px_rgba(209,122,40,0.28)]";
  const statusTextClassName = showError ? "text-danger" : "text-primary";
  const isCompactWaveState = !showIssue && (status === "recording" || status === "transcribing" || isFinishing);
  const showWarningRing = showMicWarning || (status === "idle" && missingApiKey);
  const shellClassName = `${isCompactWaveState
    ? "widget-shell widget-shell--compact relative rounded-[16px] pl-1.5 pr-0.5 py-1.5 overflow-hidden"
    : "widget-shell relative rounded-[16px] pl-2 pr-1 py-1.5 overflow-hidden"}${showWarningRing ? " widget-shell--mic-warning" : ""}`;
  const rowClassName = isCompactWaveState
    ? "flex w-full items-center pl-1 pr-0 py-1"
    : "flex w-full items-center pl-1.5 pr-0.5 py-1";
  const activeMarkSize = 22;
  const activeWaveSlotClassName = "flex h-6 w-[35px] items-center";
  const waveWrapClassName = isFinishing
    ? "widget-wave-wrap widget-wave-wrap--finish"
    : "widget-wave-wrap";
  const renderedWaveMode = waveMode === "idle" && isFinishing ? "transcribing" : waveMode;
  const widgetSizeAnchor = (
    <div className="pointer-events-none invisible">
      <div className={rowClassName}>
        <div className={`flex items-center select-none ${isCompactWaveState ? "gap-1.5" : "gap-2.5"}`}>
          {isCompactWaveState ? (
            <FamVoiceLogo size={activeMarkSize} className="shrink-0" />
          ) : (
            <FamVoiceLockup markSize={22} />
          )}
          {isCompactWaveState ? (
            <div className="h-6 w-[35px]" aria-hidden="true" />
          ) : null}
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
      <main
        ref={containerRef}
        id="widget-container"
        className={shellClassName}
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={(e) => {
          e.preventDefault();
        }}
      >
        {widgetSizeAnchor}

        <div className="absolute inset-0 flex items-center">
          {showIssue ? (
            <div className="flex w-full items-center pl-1.5 pr-0.5 py-1">
              <div className="relative flex min-w-0 flex-1 items-center select-none">
                <FamVoiceLockup aria-hidden="true" markSize={22} wordmarkClassName="opacity-0" />
                <div className="absolute inset-y-0 right-0 left-[28px] flex min-w-0 flex-col justify-center">
                  <div className="flex items-center gap-1.5">
                    <div className={`h-1 w-1 shrink-0 rounded-full ${statusDotClassName}`} />
                    <p className={`truncate text-[10px] font-bold leading-none ${statusTextClassName}`}>
                      {statusLabel}
                    </p>
                  </div>
                  <p className="truncate text-[9px] leading-tight text-slate-400">
                    {statusCopy}
                  </p>
                </div>
              </div>
            </div>
          ) : (
            <div className={rowClassName}>
              <div className={`flex items-center pointer-events-none select-none ${isCompactWaveState ? "gap-1.5" : "gap-2.5"}`}>
                {renderedWaveMode === "idle" ? (
                  <FamVoiceLockup markSize={22} />
                ) : (
                  <div className="widget-status flex min-w-0 items-center justify-center pointer-events-none select-none">
                    <FamVoiceLogo size={activeMarkSize} className="shrink-0" />
                    <div className={activeWaveSlotClassName}>
                      <div className={waveWrapClassName}>
                        <VoiceWave mode={renderedWaveMode} size="widget" />
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
