import { useEffect } from "react";
import type { MouseEventHandler, RefObject } from "react";
import { FamVoiceLogo } from "./FamVoiceLogo";
import { VoiceWave } from "./components/VoiceWave";
import type { Status } from "./appTypes";

interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  highlightKey?: number;
  errorMessage?: string;
  containerRef: RefObject<HTMLElement | null>;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
  onContextMenu: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  highlightKey,
  errorMessage,
  containerRef,
  onMouseDownCapture,
  onContextMenu,
}: WidgetViewProps) {
  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";
  const showStatusText = status === "error" || (status === "idle" && missingApiKey);
  const statusLabel = status === "error"
    ? errorMessage === "No voice detected"
      ? "No voice"
      : errorMessage || "Error"
    : "No API key";
  const statusDotClassName = status === "error"
    ? "bg-danger shadow-[0_0_10px_rgba(179,93,79,0.32)]"
    : "bg-primary animate-pulse shadow-[0_0_10px_rgba(209,122,40,0.28)]";
  const statusTextClassName = status === "error" ? "text-danger" : "text-primary";

  useEffect(() => {
    if (!highlightKey || !containerRef.current) return;
    const el = containerRef.current as HTMLElement;
    el.classList.remove("widget-highlight");
    void el.offsetHeight; // force reflow to restart animation
    el.classList.add("widget-highlight");
  }, [highlightKey, containerRef]);

  return (
    <div className="w-full h-full flex items-center justify-center p-2" style={{ pointerEvents: "none" }}>
      <main
        ref={containerRef}
        id="widget-container"
        className="widget-shell relative flex items-center gap-2.5 rounded-[18px] px-3 py-2 overflow-hidden"
        title={status === "error" ? errorMessage || "Error" : undefined}
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={onContextMenu}
      >
        <div className="pointer-events-none select-none">
          <div className="flex h-9 w-9 items-center justify-center">
            <FamVoiceLogo size={28} />
          </div>
        </div>

        <div className="widget-status flex min-w-0 items-center gap-1.5 pointer-events-none select-none">
          {showStatusText ? (
            <div className="flex min-w-0 items-center gap-1.5">
              <div className={`h-1.5 w-1.5 shrink-0 rounded-full ${statusDotClassName}`} />
              <span className={`max-w-[11rem] truncate text-[10px] font-medium whitespace-nowrap ${statusTextClassName}`}>
                {statusLabel}
              </span>
            </div>
          ) : (
            <VoiceWave mode={waveMode} size="widget" />
          )}
        </div>
      </main>
    </div>
  );
}
