import { useCallback, useEffect, useRef } from "react";
import { usePointerDragSession } from "../hooks/usePointerDragSession";
import {
  boxToEdges,
  clampBox,
  resizeFromCorner,
  type BoxRect,
  type Corner,
} from "../lib/boxGeometry";

interface TyposcopeHandlesProps {
  centerX: number;
  centerY: number;
  boxWidth: number;
  boxHeight: number;
  onBoxChange: (next: BoxRect) => void;
}

type DragMode =
  | { type: "move"; startX: number; startY: number; origin: BoxRect }
  | { type: "resize"; corner: Corner; anchor: BoxRect };

const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];

export function TyposcopeHandles({
  centerX,
  centerY,
  boxWidth,
  boxHeight,
  onBoxChange,
}: TyposcopeHandlesProps) {
  const dragRef = useRef<DragMode | null>(null);
  const onBoxChangeRef = useRef(onBoxChange);
  const { startSession, endSession } = usePointerDragSession();
  onBoxChangeRef.current = onBoxChange;

  const box = { centerX, centerY, boxWidth, boxHeight };
  const edges = boxToEdges(box);

  const handlePointerMove = useCallback((event: PointerEvent) => {
    const drag = dragRef.current;
    if (!drag) {
      return;
    }

    if (drag.type === "move") {
      const deltaX = event.clientX - drag.startX;
      const deltaY = event.clientY - drag.startY;
      onBoxChangeRef.current(
        clampBox({
          ...drag.origin,
          centerX: drag.origin.centerX + deltaX,
          centerY: drag.origin.centerY + deltaY,
        }),
      );
      return;
    }

    onBoxChangeRef.current(
      resizeFromCorner(
        drag.corner,
        event.clientX,
        event.clientY,
        drag.anchor,
      ),
    );
  }, []);

  const handlePointerUp = useCallback(() => {
    dragRef.current = null;
    endSession();
    window.removeEventListener("pointermove", handlePointerMove);
    window.removeEventListener("pointerup", handlePointerUp);
    window.removeEventListener("pointercancel", handlePointerUp);
  }, [endSession, handlePointerMove]);

  useEffect(
    () => () => {
      dragRef.current = null;
      endSession();
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    },
    [endSession, handlePointerMove, handlePointerUp],
  );

  const bindDragListeners = () => {
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);
  };

  const startMove = (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    handlePointerUp();
    startSession(event);
    dragRef.current = {
      type: "move",
      startX: event.clientX,
      startY: event.clientY,
      origin: box,
    };
    bindDragListeners();
  };

  const startResize = (
    corner: Corner,
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    handlePointerUp();
    startSession(event);
    dragRef.current = {
      type: "resize",
      corner,
      anchor: box,
    };
    bindDragListeners();
  };

  return (
    <div
      className="typoscope-handles"
      style={{
        left: `${edges.left}px`,
        top: `${edges.top}px`,
        width: `${edges.width}px`,
        height: `${edges.height}px`,
      }}
    >
      <div
        className="typoscope-handles__drag-surface"
        onPointerDown={startMove}
        aria-label="Drag typoscope"
      />

      {CORNERS.map((corner) => (
        <button
          key={corner}
          type="button"
          className={`typoscope-handles__corner typoscope-handles__corner--${corner}`}
          aria-label={`Resize ${corner} corner`}
          onPointerDown={(event) => startResize(corner, event)}
        />
      ))}
    </div>
  );
}
