import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import type { SlitRect } from "../components/TyposcopeOverlay";
import type { SnapMode } from "../lib/settings";

interface SnapResult extends SlitRect {
  mode: SnapMode;
  source: string;
  word_count: number;
  text: string;
}

export function useOcrSnap(available: boolean, enabled: boolean, mode: SnapMode) {
  const [result, setResult] = useState<SnapResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const state = useRef({ enabled, mode });
  state.current = { enabled, mode };

  useEffect(() => {
    setResult(null);
    if (!available) return;
    let disposed = false;
    const cleanups: (() => void)[] = [];
    function track(unlisten: () => void) {
      if (disposed) unlisten(); else cleanups.push(unlisten);
    }
    function failed(error: unknown) { if (!disposed) setError(String(error)); }
    void listen<SnapResult>("ocr-snap-rect", ({ payload }) => {
      if (disposed || !state.current.enabled || payload.mode !== state.current.mode) return;
      setResult(payload);
      if (payload.source === "cache" && payload.word_count > 0) setError(null);
    }).then(track).catch(failed);
    void listen<string>("ocr-snap-error", ({ payload }) => {
      if (!disposed) setError(payload);
    }).then(track).catch(failed);
    void listen("ocr-snap-clear", () => { if (!disposed) setResult(null); }).then(track).catch(failed);
    return () => { disposed = true; cleanups.forEach((fn) => fn()); };
  }, [available]);

  useEffect(() => {
    setResult(null);
    setError(null);
    if (!available) return;
    let disposed = false;
    // Issue in order: disable first, then change mode and restore active state.
    void (async () => {
      await invoke("set_auto_snap", { enabled: false });
      if (disposed) return;
      await invoke("set_snap_mode", { mode });
      if (!disposed) await invoke("set_auto_snap", { enabled });
    })().catch((error) => { if (!disposed) setError(String(error)); });
    return () => { disposed = true; };
  }, [available, enabled, mode]);

  return { result: enabled && result?.mode === mode ? result : null, error };
}
