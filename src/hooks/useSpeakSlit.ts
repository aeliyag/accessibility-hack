import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useRef, useState } from "react";
import { recognizeText } from "../lib/ocr";

export type SpeechStatus = "idle" | "capturing" | "reading" | "speaking" | "error";

interface BoxGeometry {
  centerX: number;
  centerY: number;
  boxWidth: number;
  boxHeight: number;
}

export function useSpeakSlit(geometry: BoxGeometry) {
  // Read via ref instead of a dependency so the returned `speak` callback
  // keeps a stable identity across every mouse-driven position update.
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;

  const [status, setStatus] = useState<SpeechStatus>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const busyRef = useRef(false);

  const speak = useCallback(async () => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    setErrorMessage(null);

    try {
      const { centerX, centerY, boxWidth, boxHeight } = geometryRef.current;
      const boxLeft = Math.max(0, centerX - boxWidth / 2);
      const boxTop = Math.max(0, centerY - boxHeight / 2);

      const appWindow = getCurrentWindow();
      const [scaleFactor, origin] = await Promise.all([
        appWindow.scaleFactor(),
        appWindow.outerPosition(),
      ]);

      const rect = {
        x: origin.x / scaleFactor + boxLeft,
        y: origin.y / scaleFactor + boxTop,
        width: boxWidth,
        height: boxHeight,
      };

      setStatus("capturing");
      const image = await invoke<string>("capture_slit", rect);

      setStatus("reading");
      const text = await recognizeText(image);

      if (!text.trim()) {
        throw new Error("No text detected in the box");
      }

      setStatus("speaking");
      await invoke("speak_text", { text });

      setStatus("idle");
    } catch (error) {
      setStatus("error");
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      busyRef.current = false;
    }
  }, []);

  return { status, errorMessage, speak };
}
