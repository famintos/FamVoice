import type { MouseEventHandler, RefObject } from "react";
import { FamVoiceLogo } from "./FamVoiceLogo";
import { VoiceWave } from "./components/VoiceWave";
import type { Status } from "./appTypes";

interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  updateReady: boolean;
  containerRef: RefObject<HTMLElement | null>;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
  onContextMenu: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  updateReady,
  containerRef,
  onMouseDownCapture,
  onContextMenu,
}: WidgetViewProps) {
  return (
    <div className="w-full h-full flex items-center justify-center" style={{ pointerEvents: "none" }}>
      <main
        ref={containerRef}
        id="widget-container"
        className="relative flex items-center gap-3 px-4 py-2 bg-[#0f0f13] backdrop-blur-2xl rounded-md shadow-md border border-white/10 text-white"
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={onContextMenu}
      >
        <div className="flex items-center gap-2 pointer-events-none select-none">
          <div className="flex items-center justify-center shadow-[0_0_15px_rgba(255,81,47,0.4)] rounded-full">
            <FamVoiceLogo size={24} />
          </div>
          <span className="text-[11px] font-bold tracking-wider uppercase opacity-90">Fam</span>
        </div>

        <div className="flex items-center gap-1.5 pointer-events-none">
          {status === "transcribing" && (
            <div className="w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
          )}
          {status === "error" && (
            <div className="w-2 h-2 bg-red-500 rounded-full" />
          )}
          {status === "idle" && missingApiKey && (
            <div className="w-2 h-2 bg-amber-500 rounded-full" title="API key missing — right-click to configure" />
          )}
          {status === "idle" && updateReady && (
            <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" title="Update ready — right-click to restart" />
          )}
          <VoiceWave isPlaying={status === "recording"} />
        </div>
      </main>
    </div>
  );
}
