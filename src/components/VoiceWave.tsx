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
  const isRecording = mode === "recording";
  const isTranscribing = mode === "transcribing";
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
  const motionClass = isRecording
    ? "wave-bar"
    : isTranscribing
      ? "wave-processing"
      : "transition-[opacity,transform,height] duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)]";

  return (
    <div
      className={`flex items-center ${containerClass} pointer-events-none ${isRecording ? "wave-glow" : ""}`}
    >
      {BARS.map((bar, i) => (
        <div
          key={i}
          className={`${barClass} bg-primary rounded-full ${motionClass}`}
          style={{
            height: `${bar.peak * 100}%`,
            animationDelay: isRecording ? `${bar.delay}s` : isTranscribing ? `${i * 0.08}s` : undefined,
            opacity: isTranscribing ? 0.45 : isRecording ? 1 : 0.72,
            ["--wave-duration" as "--wave-duration"]: isRecording
              ? "var(--fam-duration-fast)"
              : "var(--fam-duration-normal)",
            ["--wave-peak" as "--wave-peak"]: bar.peak,
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
