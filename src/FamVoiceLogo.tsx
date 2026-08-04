import type { ImgHTMLAttributes } from "react";
import famVoiceMarkAmber from "./assets/brand/famvoice-mark-amber.svg";
import famVoiceMarkCompactAmber from "./assets/brand/famvoice-mark-compact-amber.svg";

/**
 * At and below this size the regular stroke renders under 2px and the mark reads
 * as a smear, so the compact optical size takes over.
 */
export const COMPACT_MARK_MAX_SIZE = 32;

export interface FamVoiceLogoProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "src" | "width" | "height" | "alt"> {
  size?: number | string;
  alt?: string;
  decorative?: boolean;
}

export function markSourceForSize(size: number | string): string {
  return typeof size === "number" && size <= COMPACT_MARK_MAX_SIZE
    ? famVoiceMarkCompactAmber
    : famVoiceMarkAmber;
}

export function FamVoiceLogo({
  size = 24,
  className = "",
  alt = "FamVoice mark",
  decorative = true,
  ...rest
}: FamVoiceLogoProps) {
  return (
    <img
      {...rest}
      src={markSourceForSize(size)}
      alt={decorative ? "" : alt}
      aria-hidden={decorative || undefined}
      width={size}
      height={size}
      className={className}
      draggable={false}
    />
  );
}
