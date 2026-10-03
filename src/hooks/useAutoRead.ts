import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import { isTauri } from "../lib/isTauri";

export interface AutoReadResult {
  scrollable: boolean;
  scrolled: boolean;
  permissionDenied: boolean;
}

interface UseAutoReadOptions {
  enabled: boolean;
  linesPerMinute: number;
  onStatus: (result: AutoReadResult) => void;
}

const PIXELS_PER_LINE = 20;
const STEP_INTERVAL_MS = 50;
const TARGET_CHECK_INTERVAL_MS = 500;

export function useAutoRead({
  enabled,
  linesPerMinute,
  onStatus,
}: UseAutoReadOptions) {
  const onStatusRef = useRef(onStatus);
  onStatusRef.current = onStatus;

  useEffect(() => {
    if (!enabled || !isTauri()) {
      return;
    }

    const pixelsPerSecond = (linesPerMinute * PIXELS_PER_LINE) / 60;
    let carry = 0;
    let lastTime = performance.now();
    let lastTargetCheck = 0;
    let inFlight = false;
    let cancelled = false;

    const step = async () => {
      const now = performance.now();
      carry += (pixelsPerSecond * (now - lastTime)) / 1000;
      lastTime = now;

      if (inFlight || carry < 1) {
        return;
      }

      const pixels = Math.floor(carry);
      carry -= pixels;
      const checkTarget = now - lastTargetCheck >= TARGET_CHECK_INTERVAL_MS;
      if (checkTarget) {
        lastTargetCheck = now;
      }

      inFlight = true;
      try {
        const result = await invoke<AutoReadResult>("auto_scroll_step", {
          pixels,
          checkTarget,
        });
        if (!cancelled && checkTarget) {
          onStatusRef.current(result);
        }
      } catch {
        if (!cancelled) {
          onStatusRef.current({
            scrollable: false,
            scrolled: false,
            permissionDenied: false,
          });
        }
      } finally {
        inFlight = false;
      }
    };

    const timer = window.setInterval(() => {
      void step();
    }, STEP_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled, linesPerMinute]);
}
