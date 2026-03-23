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
  isPlaying = false,
  size = "default",
}: {
  isPlaying?: boolean;
  size?: "default" | "large";
}) {
  const containerClass = size === "large" ? "h-8 gap-[3px]" : "h-4 gap-[2px]";
  const barClass = size === "large" ? "w-[3px]" : "w-[2px]";

  return (
    <div
      className={`flex items-center justify-center ${containerClass} pointer-events-none${isPlaying ? " wave-glow" : ""}`}
    >
      {BARS.map((bar, i) => (
        <div
          key={i}
          className={`${barClass} bg-primary rounded-full ${isPlaying ? "wave-bar" : "wave-idle"}`}
          style={{
            height: `${bar.peak * 100}%`,
            animationDuration: isPlaying ? `${bar.dur}s` : "2s",
            animationDelay: isPlaying ? `${bar.delay}s` : `${i * 0.15}s`,
            "--wave-peak": bar.peak,
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
