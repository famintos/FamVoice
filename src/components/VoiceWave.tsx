import type React from "react";
import { useMemo, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

// The FamVoice mark is a short bar followed by a tall one, then the transcript
// lines. These profiles carry the same rhythm: the peak sits left of centre and
// the tail runs longer to the right, so the live wave reads as the mark moving
// rather than as a stock symmetric equalizer.
const PROFILE_PRESETS = {
  default: [0.52, 0.76, 0.94, 1, 0.93, 0.84, 0.72, 0.62, 0.5],
  widget: [0.58, 0.84, 1, 0.94, 0.82, 0.68, 0.55],
  large: [0.44, 0.62, 0.8, 0.94, 1, 0.95, 0.87, 0.78, 0.68, 0.58, 0.48],
} satisfies Record<"default" | "widget" | "large", number[]>;

type WaveBarStyle = React.CSSProperties & {
  "--bar-profile": number;
  "--bar-rest-scale": number;
  "--bar-active-scale": number;
};

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
    // Motion radiates from the loudest bar, not the geometric middle, so the
    // asymmetric profile above stays legible while the wave animates.
    const peakIndex = profiles.indexOf(Math.max(...profiles));
    const recordingBaseHeight = size === "widget" ? 24 : 20;
    const recordingRangeGain = size === "widget" ? 104 : 84;

    return profiles.map((profile, index) => {
      const distanceFromPeak = Math.abs(index - peakIndex);

      return {
        profile,
        delay: `${-(distanceFromPeak * 0.08)}s`,
        duration: `${1 + distanceFromPeak * 0.06}s`,
        restScale: 0.64 + profile * 0.14,
        activeScale: 0.9 + profile * 0.18,
        recordingHeight: `calc(${recordingBaseHeight}% + (var(--mic-level) * ${profile * recordingRangeGain}%))`,
      };
    });
  }, [size]);

  // Bar-to-gap ratio matches the mark's stroke 11 against its 10px gap, so the
  // live wave and the glyph share one rhythm.
  const containerClass = size === "large"
    ? "h-12 gap-[4px] justify-center"
    : size === "widget"
      ? "h-6 w-full justify-center gap-[3.5px] px-0.5"
      : "h-5 gap-[3px] justify-center";

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
          className={`${barWidth} voice-wave-bar shrink-0 rounded-full bg-primary transition-[opacity,height] duration-100 ease-[var(--fam-ease-ease)] ${motionClass}`}
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
            "--bar-profile": bar.profile,
            "--bar-rest-scale": bar.restScale,
            "--bar-active-scale": bar.activeScale,
            transformOrigin: "center",
          } as WaveBarStyle}
        />
      ))}
    </div>
  );
}
