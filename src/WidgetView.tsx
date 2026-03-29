import { useEffect } from "react";
import type { MouseEventHandler, RefObject } from "react";
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
  onContextMenu: MouseEventHandler<HTMLElement>;
}

export function WidgetView({
  status,
  missingApiKey,
  highlightKey,
  errorMessage,
  containerRef,
  onOpenSettings,
  onMouseDownCapture,
  onContextMenu,
}: WidgetViewProps) {
  const waveMode = status === "transcribing" ? "transcribing" : status === "recording" ? "recording" : "idle";
  const showIssue = status === "error" || (status === "idle" && missingApiKey);
  const statusLabel = status === "error" ? "Transcription error" : "Missing API key";
  const statusCopy = status === "error"
    ? errorMessage === "No voice detected"
      ? "No voice detected. Try again with a clearer input."
      : errorMessage || "Check your microphone or input source, then try again."
    : "Add your API key in Settings to start dictating.";
  const statusDotClassName = status === "error"
    ? "bg-danger shadow-[0_0_10px_rgba(179,93,79,0.32)]"
    : "bg-primary shadow-[0_0_10px_rgba(209,122,40,0.28)]";
  const statusTextClassName = status === "error" ? "text-danger" : "text-primary";
  const settingsAction = (
    <button
      type="button"
      onClick={() => {
        void onOpenSettings();
      }}
      className="focus-ring w-fit rounded-full border border-white/10 bg-black/20 px-3 py-1.5 text-sm font-medium text-white transition-colors duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)] hover:border-primary/40 hover:text-primary"
    >
      Open settings
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
        className="widget-shell relative rounded-[18px] px-2 py-2 overflow-hidden"
        style={{ pointerEvents: "auto" }}
        onMouseDownCapture={onMouseDownCapture}
        onContextMenu={onContextMenu}
      >
        {showIssue ? (
          <div className="flex flex-col gap-3 px-2.5 py-2">
            <div className="flex items-start gap-2.5">
              <FamVoiceLockup aria-hidden="true" markSize={26} wordmarkClassName="opacity-0" />
              <div className="min-w-0 flex-1 space-y-0.5">
                <div className="flex items-center gap-1.5">
                  <div className={`h-1.5 w-1.5 shrink-0 rounded-full ${statusDotClassName}`} />
                  <p className={`text-sm font-medium ${statusTextClassName}`}>
                    {statusLabel}
                  </p>
                </div>
                <p className="text-sm leading-5 text-slate-400">
                  {statusCopy}
                </p>
              </div>
            </div>

            {settingsAction}
          </div>
        ) : (
          <div className="flex flex-col gap-2 px-2.5 py-2">
            <div className="flex items-center gap-2.5 pointer-events-none select-none">
              {waveMode === "idle" ? (
                <FamVoiceLockup markSize={26} />
              ) : (
                <div className="widget-status relative flex min-w-0 items-center justify-center pointer-events-none select-none">
                  <FamVoiceLockup aria-hidden="true" markSize={26} wordmarkClassName="opacity-0" />

                  <div className="absolute inset-0 flex items-center justify-center">
                    <VoiceWave mode={waveMode} size="widget" />
                  </div>
                </div>
              )}
            </div>

            <div className="flex justify-end">
              {settingsAction}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
