import { useId } from "react";
import "./TyposcopeOverlay.css";

export interface SlitRect {
  x: number;
  y: number;
  width: number;
  height: number;
  /** Per-line openings in window logical pixels, including wrapped sentences. */
  bands?: [number, number, number, number][];
}

interface TyposcopeOverlayProps {
  centerY: number;
  slitHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  slitRect?: SlitRect | null;
}

export function TyposcopeOverlay({
  centerY, slitHeight, maskOpacity, underlayColor, underlayOpacity, slitRect,
}: TyposcopeOverlayProps) {
  const id = useId().replace(/:/g, "");
  const maskId = `reading-mask-${id}`;
  const clipId = `reading-bands-${id}`;
  const bounded = slitRect && slitRect.width > 0 && slitRect.height > 0;
  const bands = bounded ? slitRect.bands?.filter((b) => b.every(Number.isFinite) && b[2] > 0 && b[3] > 0) : undefined;
  const openings: { x: number; y: number; width: number | string; height: number }[] =
    bands?.length
      ? bands.map(([x, y, width, height]) => ({ x, y, width, height }))
      : bounded
        ? [{ x: slitRect.x, y: slitRect.y, width: slitRect.width, height: slitRect.height }]
        : [{ x: 0, y: Math.max(0, centerY - slitHeight / 2), width: "100%", height: slitHeight }];

  return (
    <div className="typoscope" aria-hidden="true">
      <svg className="typoscope__surface" width="100%" height="100%">
        <defs>
          <mask id={maskId} x="0" y="0" width="100%" height="100%"
            maskUnits="userSpaceOnUse" style={{ maskType: "luminance" }}>
            <rect width="100%" height="100%" fill="white" />
            {openings.map((band, i) => <rect key={i} {...band} fill="black" />)}
          </mask>
          <clipPath id={clipId} clipPathUnits="userSpaceOnUse">
            {openings.map((band, i) => <rect key={i} {...band} />)}
          </clipPath>
        </defs>
        <rect width="100%" height="100%" fill="black" opacity={maskOpacity} mask={`url(#${maskId})`} />
        <rect width="100%" height="100%" fill={underlayColor} opacity={underlayOpacity} clipPath={`url(#${clipId})`} />
      </svg>
    </div>
  );
}
