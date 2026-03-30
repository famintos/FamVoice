import type React from "react";
import { useMemo } from "react";

/**
 * State of the Art VoiceWave
 * Features:
 * - Robust Pill Design: Drastically fewer, thicker bars for a premium look.
 * - Symmetrical "mirrored" expansion from center.
 * - Gradient fills using brand interactive color.
 * - Distinct animations for recording (organic) and transcribing (traveling pulse).
 */

export function VoiceWave({
  mode = "idle",
  size = "default",
}: {
  mode?: "idle" | "recording" | "transcribing";
  size?: "default" | "widget" | "large";
}) {
  const isIdle = mode === "idle";
  const isRecording = mode === "recording";
  const isTranscribing = mode === "transcribing";

  // Drastically reduce bar count to allow for much thicker bars
  const barCount = size === "large" ? 14 : 9;
  
  const bars = useMemo(() => {
    return Array.from({ length: barCount }).map((_, i) => {
      // Create a smooth organic profile for the bars
      const t = i / (barCount - 1);
      // Bell-curve (Gaussian) distribution for heights - tighter for fewer bars
      const profile = Math.exp(-Math.pow(t - 0.5, 2) / 0.05);
      
      const dur = 0.7 + Math.random() * 0.5;
      const delay = -Math.random() * 2;
      return { profile, dur, delay };
    });
  }, [barCount]);

  const containerClass = size === "large"
    ? "h-12 gap-2 justify-center"
    : size === "widget"
      ? "h-6 w-full justify-around px-2"
      : "h-5 gap-1.5 justify-center";
  
  const barWidth = size === "large" 
    ? "w-2.5" // 10px
    : size === "widget" 
      ? "w-2"   // 8px
      : "w-1.5"; // 6px

  const motionClass = isRecording
    ? "wave-pulse"
    : isTranscribing
      ? "wave-shimmer"
      : "";

  return (
    <div
      className={`flex items-center ${containerClass} pointer-events-none relative ${isRecording ? "wave-bloom" : ""}`}
    >
      {bars.map((bar, i) => (
        <div
          key={i}
          className={`${barWidth} shrink-0 bg-gradient-to-t from-primary/40 via-primary to-primary/40 rounded-full transition-all duration-300 ${motionClass}`}
          style={{
            height: isIdle 
              ? `${Math.max(12, bar.profile * 30)}%` 
              : `${Math.max(15, bar.profile * 100)}%`,
            opacity: isIdle ? 0.25 : 1,
            animationDelay: isRecording 
              ? `${bar.delay}s` 
              : isTranscribing 
                ? `${i * 0.05}s` 
                : undefined,
            animationDuration: isRecording
              ? `${bar.dur * 0.9}s`
              : isTranscribing
                ? `1.2s`
                : "0s",
            animationPlayState: isIdle ? "paused" : "running",
            ["--bar-profile" as any]: bar.profile,
            transformOrigin: "center",
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
