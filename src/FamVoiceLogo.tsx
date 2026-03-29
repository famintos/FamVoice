import type { ImgHTMLAttributes } from "react";
import famintoMarkAmber from "./assets/faminto-mark-amber.svg";

export interface FamVoiceLogoProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "src" | "width" | "height" | "alt"> {
  size?: number | string;
  alt?: string;
  decorative?: boolean;
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
      src={famintoMarkAmber}
      alt={decorative ? "" : alt}
      aria-hidden={decorative || undefined}
      width={size}
      height={size}
      className={className}
      draggable={false}
    />
  );
}
