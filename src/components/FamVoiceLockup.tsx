import type { HTMLAttributes } from "react";
import { FamVoiceLogo } from "../FamVoiceLogo";

export interface FamVoiceLockupProps extends HTMLAttributes<HTMLDivElement> {
  markSize?: number | string;
  wordmarkClassName?: string;
}

export function FamVoiceLockup({
  markSize = 24,
  className = "",
  wordmarkClassName = "",
  ...rest
}: FamVoiceLockupProps) {
  const lockupClassName = [
    "inline-flex items-center gap-[var(--fam-lockup-gap)] whitespace-nowrap",
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

  return (
    <div className={lockupClassName} {...rest}>
      <FamVoiceLogo size={markSize} />
      <span className={wordmarkClasses}>
        FamVoice
        <span className="text-[var(--fam-interactive)]">.</span>
      </span>
    </div>
  );
}
