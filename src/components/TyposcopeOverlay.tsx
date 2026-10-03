import "./TyposcopeOverlay.css";

interface TyposcopeOverlayProps {
  centerY: number;
  slitHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
}

export function TyposcopeOverlay({
  centerY,
  slitHeight,
  maskOpacity,
  underlayColor,
  underlayOpacity,
}: TyposcopeOverlayProps) {
  const slitTop = Math.max(0, centerY - slitHeight / 2);
  const slitBottom = slitTop + slitHeight;

  return (
    <div className="typoscope" aria-hidden="true">
      <div
        className="typoscope__mask typoscope__mask--top"
        style={{
          height: `${slitTop}px`,
          backgroundColor: `rgba(0, 0, 0, ${maskOpacity})`,
        }}
      />
      <div
        className="typoscope__slit"
        style={{
          top: `${slitTop}px`,
          height: `${slitHeight}px`,
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
