import type React from "react";
import { useMemo, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

const PROFILE_PRESETS = {
  default: [0.42, 0.58, 0.76, 0.94, 1, 0.94, 0.76, 0.58, 0.42],
  widget: [0.42, 0.58, 0.76, 0.94, 1, 0.94, 0.76, 0.58, 0.42],
  large: [0.36, 0.48, 0.62, 0.78, 0.92, 1, 0.92, 0.78, 0.62, 0.48, 0.36],
} satisfies Record<"default" | "widget" | "large", number[]>;

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
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isRecording) {
      if (containerRef.current) {
        containerRef.current.style.setProperty("--mic-level", "0");
      }
      return;
    }

    const unlisten = listen<number>("mic-level", (event) => {
      if (containerRef.current) {
        // Use a small smoothing/dampening if needed, but for now direct
        containerRef.current.style.setProperty("--mic-level", event.payload.toString());
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [isRecording]);

  const bars = useMemo(() => {
    const profiles = PROFILE_PRESETS[size];
    const centerIndex = (profiles.length - 1) / 2;

    return profiles.map((profile, index) => {
      const distanceFromCenter = Math.abs(index - centerIndex);

      return {
        profile,
        delay: `${-(distanceFromCenter * 0.08)}s`,
        duration: `${1 + distanceFromCenter * 0.06}s`,
        restScale: 0.64 + profile * 0.14,
        activeScale: 0.9 + profile * 0.18,
      };
    });
  }, [size]);

  const containerClass = size === "large"
    ? "h-12 gap-[3px] justify-center"
    : size === "widget"
      ? "h-6 w-full justify-center gap-[1px] px-0"
      : "h-5 gap-[2px] justify-center";

  const barWidth = size === "large"
    ? "w-[4.5px]"
    : size === "widget"
      ? "w-[3px]"
      : "w-[3.5px]";

  const motionClass = isRecording
    ? "wave-bar" // Removed wave-pulse to use mic-level driven height
    : isTranscribing
      ? "wave-processing wave-shimmer"
      : "";

  return (
    <div
      ref={containerRef}
      className={`relative flex items-center ${containerClass} pointer-events-none`}
      style={{ "--mic-level": "0" } as React.CSSProperties}
    >
      {bars.map((bar, index) => (
        <div
          key={index}
          className={`${barWidth} shrink-0 rounded-full bg-primary transition-[opacity,height] duration-100 ease-[cubic-bezier(0.17,0.67,0.22,1.25)] ${motionClass}`}
          style={{
            height: isIdle
              ? `${32 + bar.profile * 16}%`
              : isRecording
                ? `calc(20% + (var(--mic-level) * ${bar.profile * 84}%))`
                : size === "widget"
                  ? `${44 + bar.profile * 30}%`
                  : `${40 + bar.profile * 42}%`,
            opacity: isIdle ? 0.3 : 0.92,
            animationDelay: isTranscribing
                ? `${index * 0.08}s`
                : undefined,
            animationDuration: isTranscribing
                ? "1.35s"
                : "0s",
            animationPlayState: isIdle ? "paused" : "running",
            ["--bar-profile" as any]: bar.profile,
            ["--bar-rest-scale" as any]: bar.restScale,
            ["--bar-active-scale" as any]: bar.activeScale,
            transformOrigin: "center",
          } as React.CSSProperties}
        />
      ))}
    </div>
  );
}
