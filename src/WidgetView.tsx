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
        className="relative flex items-center gap-2.5 px-3 py-1.5 bg-[#0f0f13] backdrop-blur-2xl rounded-2xl shadow-md border border-white/10 text-white"
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={onContextMenu}
      >
        <div className="pointer-events-none select-none">
          <div className="flex items-center justify-center shadow-[0_0_15px_rgba(255,81,47,0.4)] rounded-full">
            <FamVoiceLogo size={24} />
          </div>
        </div>

        <div className="flex items-center gap-1.5 pointer-events-none select-none">
          {status === "error" ? (
            <div className="flex items-center gap-1">
              <div className="w-1.5 h-1.5 bg-red-500 rounded-full shrink-0" />
              <span className="text-[10px] text-red-400 font-medium whitespace-nowrap">
                {errorMessage === "No voice detected" ? "No voice" : (errorMessage || "Error")}
              </span>
            </div>
          ) : status === "idle" && missingApiKey ? (
            <div className="flex items-center gap-1 pointer-events-auto group relative">
              <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 text-[10px] bg-[#1a1a2e] border border-white/15 rounded-lg whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none z-10">
                Right-click to configure API key
                <div className="absolute top-full left-1/2 -translate-x-1/2 w-0 h-0 border-l-4 border-r-4 border-t-4 border-l-transparent border-r-transparent border-t-amber-500/40" />
              </div>
              <div className="w-1.5 h-1.5 bg-amber-500 rounded-full animate-pulse shrink-0" />
              <span className="text-[10px] text-amber-400 font-medium whitespace-nowrap">No API key</span>
            </div>
          ) : (
            <VoiceWave mode={waveMode} size="widget" />
          )}
        </div>
      </main>
    </div>
  );
}
