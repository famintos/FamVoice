import type React from "react";

const BARS: { peak: number; dur: number; delay: number }[] = [
  { peak: 0.40, dur: 1.10, delay: -0.10 },
  { peak: 0.55, dur: 0.95, delay: -0.25 },
  { peak: 0.75, dur: 1.05, delay: -0.45 },
  { peak: 0.92, dur: 0.85, delay: -0.65 },
  { peak: 1.00, dur: 1.15, delay: -0.85 },
  { peak: 0.92, dur: 0.90, delay: -1.05 },
  { peak: 0.75, dur: 1.00, delay: -0.55 },
  { peak: 0.55, dur: 0.80, delay: -0.35 },
  { peak: 0.40, dur: 1.10, delay: -0.15 },
  { peak: 0.30, dur: 0.95, delay: -0.75 },
  { peak: 0.25, dur: 1.05, delay: -0.95 },
];

export function VoiceWave({
  mode = "idle",
  size = "default",
}: {
  mode?: "idle" | "recording" | "transcribing";
  size?: "default" | "widget" | "large";
}) {
  const isActiveWidget = size === "widget" && mode !== "idle";
  const containerClass = size === "large"
    ? "h-10 gap-[4px] justify-center"
    : size === "widget"
      ? isActiveWidget ? "h-6 w-[72px] justify-between" : "h-5 gap-[3px] justify-center"
      : "h-5 gap-[2.5px] justify-center";
  
  const barClass = size === "large" 
    ? "w-[4px]" 
    : size === "widget" 
      ? isActiveWidget ? "w-[3.5px]" : "w-[3px]" 
      : "w-[2.5px]";

  const isRecording = mode === "recording";
  const isTranscribing = mode === "transcribing";
  const isIdleWidget = mode === "idle" && size === "widget";

  return (
    <div
      className={`flex items-center ${containerClass} pointer-events-none ${isRecording ? "wave-glow" : ""}`}
    >
      {BARS.map((bar, i) => (
        <div
          key={i}
          className={`${barClass} bg-primary rounded-full transition-all duration-500 ${
            isRecording ? "wave-bar" : 
            isTranscribing ? "wave-processing" : 
            isIdleWidget ? "pacman-dot" : "wave-idle"
          }`}
          style={{
            height: isIdleWidget ? "3px" : `${bar.peak * 100}%`,
            animationDuration: isRecording ? `${bar.dur}s` : isTranscribing ? "1.2s" : "2.5s",
            animationDelay: isRecording ? `${bar.delay}s` : isTranscribing ? `${i * 0.1}s` : `${i * 0.2}s`,
            "--wave-peak": bar.peak,
            opacity: isTranscribing ? 0.4 : 1
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
