import { useCallback, useEffect, useRef } from "react";
import { usePointerDragSession } from "../hooks/usePointerDragSession";
import {
  MAX_SCROLL_SPEED,
  MIN_SCROLL_SPEED,
} from "../lib/settings";

interface ScrollSpeedHandleProps {
  scrollSpeed: number;
  onScrollSpeedChange: (speed: number) => void;
}

const TRACK_HEIGHT = 220;
const THUMB_SIZE = 16;

function speedToRatio(speed: number) {
  return (speed - MIN_SCROLL_SPEED) / (MAX_SCROLL_SPEED - MIN_SCROLL_SPEED);
}

function ratioToSpeed(ratio: number) {
  const clamped = Math.min(1, Math.max(0, ratio));
  return Math.round(
    MIN_SCROLL_SPEED + clamped * (MAX_SCROLL_SPEED - MIN_SCROLL_SPEED),
  );
}

export function ScrollSpeedHandle({
  scrollSpeed,
  onScrollSpeedChange,
}: ScrollSpeedHandleProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const onScrollSpeedChangeRef = useRef(onScrollSpeedChange);
  const { startSession, endSession } = usePointerDragSession();
  onScrollSpeedChangeRef.current = onScrollSpeedChange;

  const thumbTop =
    (1 - speedToRatio(scrollSpeed)) * (TRACK_HEIGHT - THUMB_SIZE);

  const handlePointerMove = useCallback((event: PointerEvent) => {
    const track = trackRef.current;
    if (!track) {
      return;
    }

    const rect = track.getBoundingClientRect();
    const ratio = 1 - (event.clientY - rect.top - 8) / (rect.height - 16);
    onScrollSpeedChangeRef.current(ratioToSpeed(ratio));
  }, []);

  const handlePointerUp = useCallback(() => {
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

  const startDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    handlePointerUp();
    startSession(event);
    handlePointerMove(event.nativeEvent);
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);
  };

  return (
    <div
      className="scroll-speed-handle"
      aria-label={`Reading speed ${scrollSpeed} lines per minute`}
    >
      <div ref={trackRef} className="scroll-speed-handle__track">
        <div
          className="scroll-speed-handle__thumb"
          style={{ top: `${thumbTop}px` }}
          onPointerDown={startDrag}
        />
      </div>
    </div>
  );
}
