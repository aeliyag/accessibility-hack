import "./TyposcopeOverlay.css";

export interface SlitRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface TyposcopeOverlayProps {
  centerY: number;
  slitHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  /** When set, draws a bounded reading window (left/right masks). */
  slitRect?: SlitRect | null;
}

export function TyposcopeOverlay({
  centerY,
  slitHeight,
  maskOpacity,
  underlayColor,
  underlayOpacity,
  slitRect,
}: TyposcopeOverlayProps) {
  const useRect = slitRect && slitRect.width > 0 && slitRect.height > 0;
  const slitTop = useRect
    ? Math.max(0, slitRect.y)
    : Math.max(0, centerY - slitHeight / 2);
  const height = useRect ? slitRect.height : slitHeight;
  const slitBottom = slitTop + height;
  const slitLeft = useRect ? Math.max(0, slitRect.x) : 0;
  const width = useRect ? slitRect.width : undefined;

  return (
    <div className="typoscope" aria-hidden="true">
      <div
        className="typoscope__mask typoscope__mask--top"
        style={{
          height: `${slitTop}px`,
          backgroundColor: `rgba(0, 0, 0, ${maskOpacity})`,
        }}
      />
      {useRect && (
        <>
          <div
            className="typoscope__mask typoscope__mask--left"
            style={{
              top: `${slitTop}px`,
              height: `${height}px`,
              width: `${slitLeft}px`,
              backgroundColor: `rgba(0, 0, 0, ${maskOpacity})`,
            }}
          />
          <div
            className="typoscope__mask typoscope__mask--right"
            style={{
              top: `${slitTop}px`,
              height: `${height}px`,
              left: `${slitLeft + (width ?? 0)}px`,
              backgroundColor: `rgba(0, 0, 0, ${maskOpacity})`,
            }}
          />
        </>
      )}
      <div
        className="typoscope__slit"
        style={{
          top: `${slitTop}px`,
          height: `${height}px`,
          left: useRect ? `${slitLeft}px` : 0,
          width: useRect ? `${width}px` : undefined,
          right: useRect ? "auto" : 0,
          backgroundColor: underlayColor,
          opacity: underlayOpacity,
        }}
      />
      <div
        className="typoscope__mask typoscope__mask--bottom"
        style={{
          top: `${slitBottom}px`,
          backgroundColor: `rgba(0, 0, 0, ${maskOpacity})`,
        }}
      />
    </div>
  );
}
