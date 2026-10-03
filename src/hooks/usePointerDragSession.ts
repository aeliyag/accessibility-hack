import { useCallback, useEffect, useRef } from "react";
import { beginPointerDrag } from "../lib/pointerDragLock";

export function usePointerDragSession() {
  const endDragRef = useRef<(() => void) | null>(null);

  const endSession = useCallback(() => {
    endDragRef.current?.();
    endDragRef.current = null;
  }, []);

  const startSession = useCallback((_event: React.PointerEvent<Element>) => {
    endSession();
    endDragRef.current = beginPointerDrag();
  }, [endSession]);

  useEffect(() => () => endSession(), [endSession]);

  return { startSession, endSession };
}
