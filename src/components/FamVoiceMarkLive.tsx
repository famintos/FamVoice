import type React from "react";
import { useCallback, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * The FamVoice mark, alive.
 *
 * The mark is amplitude in, transcript out, so each half does the job it stands
 * for: the bars carry live microphone level, the lines carry the transcript.
 *
 * The bars rest at the mark's own geometry and grow upward from there. They never
 * shrink below it — a shape shorter than it is wide reads as a dot, and a row of
 * dots is not a mark.
 *
 * Bars are rounded rects whose height is written straight from the level samples.
 * They were once stroked lines driven by `scaleY`, which squashed the round caps,
 * and the `non-scaling-stroke` fix for that quietly decoupled bar weight from the
 * viewBox: at 46px the bars rendered 15px wide against 6.9px lines. Geometry in
 * user units keeps both halves of the mark at one weight at every size.
 */

const MIC_LEVEL_GAIN = 1.35;

export const STROKE = 15;

/**
 * `halfRest` is the mark's canonical half-height; `halfMax` is where a loud sample
 * takes it. `halfRest * 2` must stay comfortably above STROKE, or a silent bar
 * turns into a bead.
 */
export const BARS = [
  { x: 12, halfRest: 18, halfMax: 34 },
  { x: 35, halfRest: 28, halfMax: 40 },
];

const LINES = [
  { y: 33, x2: 87 },
  { y: 67, x2: 77 },
];

const LINE_X = 58;
const CENTER = 50;

type LineStyle = React.CSSProperties & { "--line-length": number };

export function FamVoiceMarkLive({
  mode,
  className = "",
}: {
  /**
   * `idle` is the mark at rest: same geometry, no listener. The widget keeps the
   * live mark mounted through idle so it can cross-fade with the static one
   * instead of popping in at the moment recording starts.
   */
  mode: "recording" | "transcribing" | "idle";
  className?: string;
}) {
  const barRefs = useRef<(SVGRectElement | null)[]>([]);
  const isRecording = mode === "recording";
  const modeClassName =
    mode === "recording"
      ? "mark-live--recording"
      : mode === "transcribing"
        ? "mark-live--transcribing"
        : "mark-live--idle";

  const applyLevel = useCallback((level: number) => {
    const clamped = Math.min(1, Math.max(0, level));

    BARS.forEach((bar, index) => {
      const rect = barRefs.current[index];
      if (!rect) return;

      const half = bar.halfRest + clamped * (bar.halfMax - bar.halfRest);
      rect.setAttribute("y", String(CENTER - half));
      rect.setAttribute("height", String(half * 2));
    });
  }, []);

  useEffect(() => {
    if (!isRecording) {
      applyLevel(0);
      return;
    }

    const unlisten = listen<number>("mic-level", (event) => {
      applyLevel(event.payload * MIC_LEVEL_GAIN);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [applyLevel, isRecording]);

  return (
    <svg
      viewBox="0 0 100 100"
      className={`mark-live ${modeClassName} text-primary ${className}`}
      fill="none"
      aria-hidden="true"
      focusable="false"
    >
      {BARS.map((bar, index) => (
        <rect
          key={bar.x}
          ref={(element) => {
            barRefs.current[index] = element;
          }}
          className="mark-live-bar"
          x={bar.x - STROKE / 2}
          y={CENTER - bar.halfRest}
          width={STROKE}
          height={bar.halfRest * 2}
          rx={STROKE / 2}
          fill="currentColor"
        />
      ))}

      {LINES.map((line, index) => (
        <line
          key={line.y}
          className="mark-live-line"
          x1={LINE_X}
          x2={line.x2}
          y1={line.y}
          y2={line.y}
          stroke="currentColor"
          strokeWidth={STROKE}
          strokeLinecap="round"
          style={
            {
              "--line-length": line.x2 - LINE_X,
              animationDelay: `${index * 0.12}s`,
            } as LineStyle
          }
        />
      ))}
    </svg>
  );
}
