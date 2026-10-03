import { useLayoutEffect, useRef } from "react";
import { boxToEdges } from "../lib/boxGeometry";
import { ScrollSpeedHandle } from "./ScrollSpeedHandle";
import { TyposcopeHandles } from "./TyposcopeHandles";
import "./TyposcopeOverlay.css";

interface TyposcopeOverlayProps {
  centerX: number;
  centerY: number;
  boxWidth: number;
  boxHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  scrollSpeed: number;
  editMode: boolean;
  autoScroll: boolean;
  onBoxChange: (next: {
    centerX: number;
    centerY: number;
    boxWidth: number;
    boxHeight: number;
  }) => void;
  onScrollSpeedChange: (speed: number) => void;
}

export function TyposcopeOverlay({
  centerX,
  centerY,
  boxWidth,
  boxHeight,
  maskOpacity,
  underlayColor,
  underlayOpacity,
  scrollSpeed,
  editMode,
  autoScroll,
  onBoxChange,
  onScrollSpeedChange,
}: TyposcopeOverlayProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const edges = boxToEdges({ centerX, centerY, boxWidth, boxHeight });
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const pixelRatio = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(viewportWidth * pixelRatio);
    canvas.height = Math.ceil(viewportHeight * pixelRatio);

    const context = canvas.getContext("2d", { alpha: true });
    if (!context) {
      return;
    }

    context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);

    // Replace every pixel in the backing bitmap. The small alpha floor is
    // visually imperceptible, but makes macOS replace transparent pixels
    // instead of preserving a previous translucent frame.
    context.globalCompositeOperation = "copy";
    context.fillStyle = "rgba(0, 0, 0, 0.01)";
    context.fillRect(0, 0, viewportWidth, viewportHeight);
    context.globalCompositeOperation = "source-over";

    // Edit mode uses only a crisp geometry preview. The translucent mask and
    // underlay are restored after editing finishes, avoiding tint trails.
    if (editMode) {
      context.strokeStyle = "rgba(255, 255, 255, 0.95)";
      context.lineWidth = 2;
      context.strokeRect(
        edges.left + 1,
        edges.top + 1,
        Math.max(0, edges.width - 2),
        Math.max(0, edges.height - 2),
      );
      return;
    }

    context.fillStyle = `rgba(0, 0, 0, ${maskOpacity})`;
    context.fillRect(0, 0, viewportWidth, edges.top);
    context.fillRect(
      0,
      edges.bottom,
      viewportWidth,
      viewportHeight - edges.bottom,
    );
    context.fillRect(0, edges.top, edges.left, edges.height);
    context.fillRect(
      edges.right,
      edges.top,
      viewportWidth - edges.right,
      edges.height,
    );

    context.globalAlpha = underlayOpacity;
    context.fillStyle = underlayColor;
    context.fillRect(edges.left, edges.top, edges.width, edges.height);
    context.globalAlpha = 1;
  }, [
    edges.bottom,
    edges.height,
    edges.left,
    edges.right,
    edges.top,
    edges.width,
    editMode,
    maskOpacity,
    underlayColor,
    underlayOpacity,
    viewportHeight,
    viewportWidth,
  ]);

  return (
    <div
      className={`typoscope ${editMode ? "typoscope--edit" : ""} ${autoScroll ? "typoscope--autoscroll" : ""}`}
    >
      <canvas
        ref={canvasRef}
        className="typoscope__surface"
        aria-hidden="true"
      />

      {editMode && (
        <>
          <TyposcopeHandles
            centerX={centerX}
            centerY={centerY}
            boxWidth={boxWidth}
            boxHeight={boxHeight}
            onBoxChange={onBoxChange}
          />
          <ScrollSpeedHandle
            scrollSpeed={scrollSpeed}
            onScrollSpeedChange={onScrollSpeedChange}
          />
        </>
      )}
    </div>
  );
}
