import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import type { Point } from "./useMousePosition";

interface WindowFrame {
  originX: number;
  originY: number;
  scaleFactor: number;
}

async function readWindowFrame(): Promise<WindowFrame> {
  const appWindow = getCurrentWindow();
  const scaleFactor = await appWindow.scaleFactor();
  const origin = await appWindow.innerPosition();

  return {
    originX: origin.x,
    originY: origin.y,
    scaleFactor,
  };
}

function toWindowPoint(
  screenX: number,
  screenY: number,
  frame: WindowFrame,
): Point {
  // device_query reports macOS/global coords in logical points; window origin is physical.
  const originX = frame.originX / frame.scaleFactor;
  const originY = frame.originY / frame.scaleFactor;

  return {
    x: screenX - originX,
    y: screenY - originY,
  };
}

export function useGlobalMouse(enabled: boolean): Point | null {
  const [position, setPosition] = useState<Point | null>(null);
  const frameRef = useRef<WindowFrame>({
    originX: 0,
    originY: 0,
    scaleFactor: 1,
  });

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const appWindow = getCurrentWindow();
    void appWindow.setAlwaysOnTop(true);

    void readWindowFrame().then((frame) => {
      frameRef.current = frame;
    });

    const refreshFrame = () => {
      void readWindowFrame().then((frame) => {
        frameRef.current = frame;
      });
    };

    window.addEventListener("resize", refreshFrame);
    const frameTimer = window.setInterval(refreshFrame, 1000);

    let unlisten: (() => void) | undefined;

    void listen<{ x: number; y: number }>("device-mouse-move", ({ payload }) => {
      setPosition(toWindowPoint(payload.x, payload.y, frameRef.current));
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
      window.removeEventListener("resize", refreshFrame);
      window.clearInterval(frameTimer);
    };
  }, [enabled]);

  return position;
}
