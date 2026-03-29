import type React from "react";

const BARS: { peak: number; dur: number; delay: number }[] = [
  { peak: 0.50, dur: 1.00, delay: -0.10 },
  { peak: 0.72, dur: 0.88, delay: -0.35 },
  { peak: 0.92, dur: 1.10, delay: -0.60 },
  { peak: 1.00, dur: 0.90, delay: -0.85 },
  { peak: 0.92, dur: 1.05, delay: -0.45 },
  { peak: 0.72, dur: 0.85, delay: -0.20 },
  { peak: 0.50, dur: 1.00, delay: -0.70 },
];

export function VoiceWave({
  mode = "idle",
  size = "default",
}: {
  mode?: "idle" | "recording" | "transcribing";
  size?: "default" | "widget" | "large";
}) {
  const containerClass = size === "large"
    ? "h-8 gap-[3px]"
    : size === "widget"
      ? "h-5 gap-[2.5px]"
      : "h-4 gap-[2px]";
  const barClass = size === "large" ? "w-[3px]" : size === "widget" ? "w-[2.5px]" : "w-[2px]";
  const isRecording = mode === "recording";
  const isTranscribing = mode === "transcribing";
  const isIdleWidget = mode === "idle" && size === "widget";

  return (
    <div
      className={`flex items-center justify-center ${containerClass} pointer-events-none${isRecording ? " wave-glow" : ""}`}
    >
      {BARS.map((bar, i) => (
        <div
          key={i}
          className={`${barClass} bg-primary rounded-full ${isRecording ? "wave-bar" : isTranscribing ? "wave-processing" : isIdleWidget ? "pacman-dot" : "wave-idle"}`}
          style={{
            height: isIdleWidget ? "2.5px" : `${bar.peak * 100}%`,
            animationDuration: isRecording ? `${bar.dur}s` : isTranscribing ? `${1.6 + bar.peak * 0.2}s` : isIdleWidget ? "1.5s" : "2s",
            animationDelay: isRecording ? `${bar.delay}s` : isTranscribing ? `${i * 0.12}s` : isIdleWidget ? `${(BARS.length - 1 - i) * 0.15}s` : `${i * 0.15}s`,
            "--wave-peak": bar.peak,
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
