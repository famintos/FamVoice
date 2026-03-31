import type React from "react";
import { useMemo, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

const PROFILE_PRESETS = {
  default: [0.42, 0.58, 0.76, 0.94, 1, 0.94, 0.76, 0.58, 0.42],
  widget: [0.5, 0.74, 0.94, 1, 0.94, 0.74, 0.5],
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
  const micLevelGain = size === "widget" ? 1.35 : 1;

  useEffect(() => {
    const setMicLevel = (nextLevel: number) => {
      if (!containerRef.current) return;

      const adjustedLevel = Math.min(1, nextLevel * micLevelGain);
      containerRef.current.style.setProperty("--mic-level", adjustedLevel.toString());
    };

    if (!isRecording) {
      setMicLevel(0);
      return;
    }

    const unlisten = listen<number>("mic-level", (event) => {
      setMicLevel(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [isRecording, micLevelGain]);

  const bars = useMemo(() => {
    const profiles = PROFILE_PRESETS[size];
    const centerIndex = (profiles.length - 1) / 2;
    const recordingBaseHeight = size === "widget" ? 24 : 20;
    const recordingRangeGain = size === "widget" ? 104 : 84;

    return profiles.map((profile, index) => {
      const distanceFromCenter = Math.abs(index - centerIndex);

      return {
        profile,
        delay: `${-(distanceFromCenter * 0.08)}s`,
        duration: `${1 + distanceFromCenter * 0.06}s`,
        restScale: 0.64 + profile * 0.14,
        activeScale: 0.9 + profile * 0.18,
        recordingHeight: `calc(${recordingBaseHeight}% + (var(--mic-level) * ${profile * recordingRangeGain}%))`,
      };
    });
  }, [size]);

  const containerClass = size === "large"
    ? "h-12 gap-[3px] justify-center"
    : size === "widget"
      ? "h-6 w-full justify-center gap-[2px] px-0.5"
      : "h-5 gap-[2px] justify-center";

  const barWidth = size === "large"
    ? "w-[4.5px]"
    : size === "widget"
      ? "w-[4px]"
      : "w-[3.5px]";

  const motionClass = isRecording
    ? "wave-bar"
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
                ? bar.recordingHeight
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
