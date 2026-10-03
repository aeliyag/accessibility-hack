import { useCallback, useEffect, useRef } from "react";
import { usePointerDragSession } from "../hooks/usePointerDragSession";
import "./ControlsPanel.css";

interface ControlsPanelProps {
  visible: boolean;
  x: number;
  y: number;
  onPositionChange: (x: number, y: number) => void;
}

const SHORTCUTS = [
  { keys: "Shift+X", action: "Pause / resume mouse follow" },
  { keys: "Shift+M", action: "Toggle edit mode (drag, resize, scroll speed)" },
  { keys: "Shift+R", action: "Auto-read — scroll content under cursor" },
  { keys: "Shift+H", action: "Show / hide this panel" },
  { keys: "↑ / ↓", action: "Nudge reading box" },
  { keys: "Shift+[ / ]", action: "Mask opacity" },
  { keys: "1", action: "Cycle underlay color" },
  { keys: "D", action: "Debug HUD" },
];

function clampPosition(x: number, y: number, width: number, height: number) {
  const maxX = Math.max(8, window.innerWidth - width - 8);
  const maxY = Math.max(8, window.innerHeight - height - 8);
  return {
    x: Math.min(maxX, Math.max(8, x)),
    y: Math.min(maxY, Math.max(8, y)),
  };
}

export function ControlsPanel({
  visible,
  x,
  y,
  onPositionChange,
}: ControlsPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const dragOffsetRef = useRef<{ x: number; y: number } | null>(null);
  const { startSession, endSession } = usePointerDragSession();

  const handlePointerMove = useCallback(
    (event: PointerEvent) => {
      const offset = dragOffsetRef.current;
      const panel = panelRef.current;
      if (!offset || !panel) {
        return;
      }

      const rect = panel.getBoundingClientRect();
      const next = clampPosition(
        event.clientX - offset.x,
        event.clientY - offset.y,
        rect.width,
        rect.height,
      );
      onPositionChange(next.x, next.y);
    },
    [onPositionChange],
  );

  const handlePointerUp = useCallback(() => {
    dragOffsetRef.current = null;
    endSession();
    window.removeEventListener("pointermove", handlePointerMove);
    window.removeEventListener("pointerup", handlePointerUp);
    window.removeEventListener("pointercancel", handlePointerUp);
  }, [endSession, handlePointerMove]);

  useEffect(
    () => () => {
      endSession();
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    },
    [endSession, handlePointerMove, handlePointerUp],
  );

  const startDrag = (event: React.PointerEvent<HTMLParagraphElement>) => {
    event.preventDefault();
    event.stopPropagation();
    handlePointerUp();
    startSession(event);
    dragOffsetRef.current = {
      x: event.clientX - x,
      y: event.clientY - y,
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);
  };

  if (!visible) {
    return null;
  }

  return (
    <div
      ref={panelRef}
      className="controls-panel"
      style={{ left: `${x}px`, top: `${y}px` }}
      aria-label="Keyboard shortcuts"
    >
      <p
        className="controls-panel__title"
        onPointerDown={startDrag}
        title="Drag to reposition"
      >
        Controls
      </p>
      <ul className="controls-panel__list">
        {SHORTCUTS.map((item) => (
          <li key={item.keys}>
            <span className="controls-panel__keys">{item.keys}</span>
            <span className="controls-panel__action">{item.action}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
