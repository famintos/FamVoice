import { useEffect } from "react";
import type { MouseEventHandler, RefObject } from "react";
import { Settings } from "lucide-react";
import { FamVoiceLockup } from "./components/FamVoiceLockup";
import { VoiceWave } from "./components/VoiceWave";
import type { Status } from "./appTypes";

interface WidgetViewProps {
  status: Status;
  missingApiKey: boolean;
  highlightKey?: number;
  errorMessage?: string;
  containerRef: RefObject<HTMLElement | null>;
  onOpenSettings: () => void;
  onMouseDownCapture: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  highlightKey,
  errorMessage,
  containerRef,
  onOpenSettings,
  onMouseDownCapture,
}: WidgetViewProps) {
  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";
  const showIssue = status === "error" || (status === "idle" && missingApiKey);
  const statusLabel = status === "error" ? "Transcription error" : "Missing API key";
  const statusCopy = status === "error"
    ? errorMessage === "No voice detected"
      ? "No voice detected."
      : errorMessage || "Transcription failed."
    : "Add API key in settings.";
  const statusDotClassName = status === "error"
    ? "bg-danger shadow-[0_0_10px_rgba(179,93,79,0.32)]"
    : "bg-primary shadow-[0_0_10px_rgba(209,122,40,0.28)]";
  const statusTextClassName = status === "error" ? "text-danger" : "text-primary";
  const settingsAction = (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        void onOpenSettings();
      }}
      className="focus-ring flex h-6 w-6 items-center justify-center rounded-full border border-white/10 bg-black/20 text-slate-400 transition-colors duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)] hover:border-primary/40 hover:text-white no-drag"
      aria-label="Settings"
    >
      <Settings size={11} />
    </button>
  );

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
        className="widget-shell relative rounded-[16px] px-2 py-1.5 overflow-hidden"
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={(e) => {
          e.preventDefault();
        }}
      >
        {showIssue ? (
          <div className="flex items-center gap-3 px-1.5 py-1">
            <div className="relative flex items-center select-none">
              <FamVoiceLockup aria-hidden="true" markSize={22} wordmarkClassName="opacity-0" />
              <div className="absolute inset-y-0 right-0 left-[28px] flex flex-col justify-center min-w-0">
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

            <div className="ml-1">
              {settingsAction}
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-3 px-1.5 py-1">
            <div className="flex items-center gap-2.5 pointer-events-none select-none">
              {waveMode === "idle" ? (
                <FamVoiceLockup markSize={22} />
              ) : (
                <div className="widget-status relative flex min-w-0 items-center justify-center pointer-events-none select-none">
                  <FamVoiceLockup aria-hidden="true" markSize={22} wordmarkClassName="opacity-0" />

                  <div className="absolute inset-0 flex items-center justify-center">
                    <VoiceWave mode={waveMode} size="widget" />
                  </div>
                </div>
              )}
            </div>

            <div className="ml-1">
              {settingsAction}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
