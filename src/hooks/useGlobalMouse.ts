import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState } from "react";
import type { Point } from "./useMousePosition";

async function toWindowPoint(x: number, y: number): Promise<Point> {
  const appWindow = getCurrentWindow();
  const scaleFactor = await appWindow.scaleFactor();
  const origin = await appWindow.outerPosition();

  return {
    x: x / scaleFactor - origin.x / scaleFactor,
    y: y / scaleFactor - origin.y / scaleFactor,
  };
}

export function useGlobalMouse(enabled: boolean): Point | null {
  const [position, setPosition] = useState<Point | null>(null);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const webview = getCurrentWebviewWindow();
    const appWindow = getCurrentWindow();

    void appWindow.setAlwaysOnTop(true);
    void appWindow.setIgnoreCursorEvents(true);

    let unlisten: (() => void) | undefined;

    void listen<{ x: number; y: number }>("device-mouse-move", ({ payload }) => {
      void toWindowPoint(payload.x, payload.y).then(setPosition);
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
      void webview.setIgnoreCursorEvents(false);
    };
  }, [enabled]);

  return position;
}
