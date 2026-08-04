import type { HTMLAttributes, ReactNode } from "react";
import { FamVoiceLogo } from "../FamVoiceLogo";

export interface FamVoiceLockupProps extends HTMLAttributes<HTMLDivElement> {
  markSize?: number | string;
  motion?: "none" | "fade-in";
  wordmarkClassName?: string;
  /**
   * Lets the wordmark collapse out of the lockup so the mark can carry the
   * surface alone. The widget uses it: the name is worth its pixels while the
   * app is waiting, not while it is listening.
   */
  collapsible?: boolean;
  collapsed?: boolean;
  /** Replaces the static mark — the widget swaps in its live one. */
  mark?: ReactNode;
}

export function FamVoiceLockup({
  markSize = 24,
  className = "",
  motion = "none",
  wordmarkClassName = "",
  collapsible = false,
  collapsed = false,
  mark,
  ...rest
}: FamVoiceLockupProps) {
  const lockupClassName = [
    "inline-flex items-center gap-[var(--fam-lockup-gap)] whitespace-nowrap",
    // A collapsible lockup carries the gap on the wordmark itself, so the gap
    // collapses with the word instead of leaving a hole beside the mark.
    collapsible ? "lockup--collapsible" : "",
    motion === "fade-in" ? "lockup-motion--fade-in" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  const wordmarkClasses = [
    "inline-flex items-baseline font-sans text-[var(--fam-type-base)] font-bold leading-none tracking-[var(--fam-letter-spacing)] text-[var(--fam-text-primary)]",
    wordmarkClassName,
  ]
    .filter(Boolean)
    .join(" ");

  const wordmark = (
    <span className={wordmarkClasses}>
      FamVoice
      <span className="text-[var(--fam-interactive)]">.</span>
    </span>
  );

  return (
    <div
      className={lockupClassName}
      data-collapsed={collapsible ? collapsed : undefined}
      {...rest}
    >
      {mark ?? <FamVoiceLogo size={markSize} />}
      {collapsible ? <span className="lockup-wordmark-slot">{wordmark}</span> : wordmark}
    </div>
  );
}
