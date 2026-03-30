import type React from "react";

const BARS: { peak: number; dur: number; delay: number }[] = [
  { peak: 0.25, dur: 1.05, delay: -0.95 },
  { peak: 0.30, dur: 0.95, delay: -0.75 },
  { peak: 0.40, dur: 1.10, delay: -0.10 },
  { peak: 0.50, dur: 1.00, delay: -0.20 },
  { peak: 0.65, dur: 0.90, delay: -0.35 },
  { peak: 0.80, dur: 1.15, delay: -0.50 },
  { peak: 0.92, dur: 0.85, delay: -0.65 },
  { peak: 0.98, dur: 1.05, delay: -0.75 },
  { peak: 1.00, dur: 1.15, delay: -0.85 },
  { peak: 0.98, dur: 0.95, delay: -0.95 },
  { peak: 0.92, dur: 0.90, delay: -1.05 },
  { peak: 0.80, dur: 1.00, delay: -1.20 },
  { peak: 0.65, dur: 1.10, delay: -1.35 },
  { peak: 0.50, dur: 0.85, delay: -1.50 },
  { peak: 0.40, dur: 0.95, delay: -1.65 },
  { peak: 0.30, dur: 1.05, delay: -1.80 },
  { peak: 0.25, dur: 0.90, delay: -1.95 },
  { peak: 0.20, dur: 1.15, delay: -2.10 },
  { peak: 0.15, dur: 1.00, delay: -2.25 },
];

export function VoiceWave({
  mode = "idle",
  size = "default",
}: {
  mode?: "idle" | "recording" | "transcribing";
  size?: "default" | "widget" | "large";
}) {
  const isActiveWidget = size === "widget" && mode !== "idle";
  const isRecording = mode === "recording";
  const isTranscribing = mode === "transcribing";
  const isIdle = mode === "idle";

  const containerClass = size === "large"
    ? "h-7 gap-[3px] justify-center"
    : size === "widget"
      ? isActiveWidget ? "h-6 w-[110px] justify-between" : "h-5 gap-[3px] justify-center"
      : "h-5 gap-[2.5px] justify-center";
  
  const barClass = size === "large" 
    ? "w-[3px]" 
    : size === "widget" 
      ? isActiveWidget ? "w-[3.5px]" : "w-[3px]" 
      : "w-[2.5px]";
  const motionClass = isRecording
    ? "wave-bar"
    : isTranscribing
      ? "wave-processing"
      : "";

  return (
    <div
      className={`flex items-center ${containerClass} pointer-events-none ${isRecording ? "wave-glow" : ""}`}
    >
      {BARS.map((bar, i) => (
        <div
          key={i}
          className={`${barClass} bg-primary rounded-full ${motionClass}`}
          style={{
            height: isIdle ? `${bar.peak * 35}%` : `${bar.peak * 100}%`,
            animationDelay: isRecording ? `${bar.delay}s` : isTranscribing ? `${i * 0.08}s` : undefined,
            animationPlayState: isIdle ? "paused" : "running",
            opacity: isIdle ? 0.35 : isTranscribing ? 0.5 : 1,
            ["--wave-duration" as "--wave-duration"]: isRecording
              ? `${bar.dur * 0.8}s`
              : isTranscribing
                ? `${bar.dur * 1.2}s`
                : "0s",
            ["--wave-peak" as "--wave-peak"]: bar.peak,
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
