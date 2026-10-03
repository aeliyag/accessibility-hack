import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import {
  TyposcopeOverlay,
  type SlitRect,
} from "./components/TyposcopeOverlay";
import { useGlobalMouse } from "./hooks/useGlobalMouse";
import { useMousePosition } from "./hooks/useMousePosition";
import {
  useSmoothedPosition,
  useSmoothedSnapRect,
} from "./hooks/useSmoothedPosition";
import { useTyposcopeSettings } from "./hooks/useTyposcopeSettings";
import {
  COLOR_PRESETS,
  cycleSnapMode,
  type SnapMode,
} from "./lib/settings";
import { isTauri } from "./lib/isTauri";
import "./App.css";

const MIN_SLIT_HEIGHT = 24;
const MAX_SLIT_HEIGHT = 240;
const SLIT_HEIGHT_STEP = 8;
const OPACITY_STEP = 0.05;
const NUDGE_STEP = 8;

const MODE_KEYS: Record<string, SnapMode> = {
  w: "word",
  l: "line",
  s: "sentence",
  p: "paragraph",
};

function App() {
  const overlayMode = isTauri();
  const [showDebug, setShowDebug] = useState(false);
  const [snapTarget, setSnapTarget] = useState<SlitRect | null>(null);
  const [snapError, setSnapError] = useState<string | null>(null);
  const [snapMeta, setSnapMeta] = useState<{ source?: string; latencyMs?: number }>(
    {},
  );
  const { settings, loaded, updateSettings } = useTyposcopeSettings();
  const localMouse = useMousePosition();
  const globalMouse = useGlobalMouse(overlayMode && loaded);
  const mouse = overlayMode ? globalMouse : localMouse;
  const position = useSmoothedPosition(
    mouse,
    settings.slitHeight,
    settings.yOffset,
  );
  const { rect: snapRect } = useSmoothedSnapRect(
    settings.autoSnap ? snapTarget : null,
    0.92,
  );

  const applyControlKey = useCallback(
    (key: string, isRepeat = false) => {
      if (key === "arrowup") {
        updateSettings((current) => ({
          ...current,
          yOffset: current.yOffset - NUDGE_STEP,
        }));
      } else if (key === "arrowdown") {
        updateSettings((current) => ({
          ...current,
          yOffset: current.yOffset + NUDGE_STEP,
        }));
      } else if (key === "g") {
        updateSettings((current) => ({
          ...current,
          slitHeight: Math.min(
            MAX_SLIT_HEIGHT,
            current.slitHeight + SLIT_HEIGHT_STEP,
          ),
        }));
      } else if (key === "h") {
        updateSettings((current) => ({
          ...current,
          slitHeight: Math.max(
            MIN_SLIT_HEIGHT,
            current.slitHeight - SLIT_HEIGHT_STEP,
          ),
        }));
      } else if (key === "[") {
        updateSettings((current) => ({
          ...current,
          maskOpacity: Math.max(0.1, current.maskOpacity - OPACITY_STEP),
        }));
      } else if (key === "]") {
        updateSettings((current) => ({
          ...current,
          maskOpacity: Math.min(0.95, current.maskOpacity + OPACITY_STEP),
        }));
      } else if (key === "1" && !isRepeat) {
        updateSettings((current) => {
          const nextIndex = (current.colorPresetIndex + 1) % COLOR_PRESETS.length;
          return {
            ...current,
            colorPresetIndex: nextIndex,
            underlayColor: COLOR_PRESETS[nextIndex].underlayColor,
          };
        });
      } else if (key === "t" && !isRepeat) {
        updateSettings((current) => ({
          ...current,
          visible: !current.visible,
        }));
      } else if (key === "d" && !isRepeat) {
        setShowDebug((value) => !value);
      } else if (key === "shifta" && !isRepeat) {
        updateSettings((current) => ({
          ...current,
          autoSnap: !current.autoSnap,
        }));
      } else if (!isRepeat && (key === "w" || key === "l" || key === "s" || key === "p" || key === "m")) {
        // Snap-mode hotkeys only while auto-snap is on.
        updateSettings((current) => {
          if (!current.autoSnap) {
            return current;
          }
          if (key === "m") {
            return { ...current, snapMode: cycleSnapMode(current.snapMode) };
          }
          const next = MODE_KEYS[key];
          return next ? { ...current, snapMode: next } : current;
        });
      }
    },
    [updateSettings],
  );

  useEffect(() => {
    if (!overlayMode || !loaded) {
      return;
    }

    const appWindow = getCurrentWindow();
    if (settings.visible) {
      void appWindow.show();
    } else {
      void appWindow.hide();
    }
  }, [loaded, overlayMode, settings.visible]);

  // Sync auto-snap flag into the Rust OCR loop.
  useEffect(() => {
    if (!overlayMode || !loaded) {
      return;
    }

    void invoke("set_auto_snap", { enabled: settings.autoSnap }).catch(() => {
      /* command unavailable off-macOS */
    });

    if (!settings.autoSnap) {
      setSnapTarget(null);
      setSnapError(null);
    }
  }, [loaded, overlayMode, settings.autoSnap]);

  // Sync snap granularity into Rust.
  useEffect(() => {
    if (!overlayMode || !loaded) {
      return;
    }

    void invoke("set_snap_mode", { mode: settings.snapMode }).catch(() => {
      /* command unavailable off-macOS */
    });
  }, [loaded, overlayMode, settings.snapMode]);

  useEffect(() => {
    if (!overlayMode) {
      return;
    }

    let unlistenRect: (() => void) | undefined;
    let unlistenErr: (() => void) | undefined;
    let unlistenClear: (() => void) | undefined;

    void listen<
      SlitRect & { word_count?: number; source?: string; latency_ms?: number }
    >("ocr-snap-rect", ({ payload }) => {
      setSnapTarget({
        x: payload.x,
        y: payload.y,
        width: payload.width,
        height: payload.height,
      });
      setSnapMeta({
        source: payload.source,
        latencyMs: payload.latency_ms,
      });
      setSnapError(null);
    }).then((cleanup) => {
      unlistenRect = cleanup;
    });

    void listen<string>("ocr-snap-error", ({ payload }) => {
      setSnapError(payload);
    }).then((cleanup) => {
      unlistenErr = cleanup;
    });

    void listen("ocr-snap-clear", () => {
      setSnapTarget(null);
      setSnapMeta({});
    }).then((cleanup) => {
      unlistenClear = cleanup;
    });

    return () => {
      unlistenRect?.();
      unlistenErr?.();
      unlistenClear?.();
    };
  }, [overlayMode]);

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      if (event.shiftKey && event.key.toLowerCase() === "a") {
        event.preventDefault();
        applyControlKey("shifta", event.repeat);
        return;
      }

      applyControlKey(event.key.toLowerCase(), event.repeat);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [applyControlKey, overlayMode]);

  useEffect(() => {
    if (!overlayMode) {
      return;
    }

    let unlisten: (() => void) | undefined;

    void listen<string>("device-key-down", ({ payload }) => {
      applyControlKey(payload);
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => unlisten?.();
  }, [applyControlKey, overlayMode]);

  if (!loaded) {
    return null;
  }

  const showSnapSlit = settings.autoSnap && snapRect;

  return (
    <div className={`app ${overlayMode ? "app--overlay" : ""}`}>
      {settings.visible && (
        <TyposcopeOverlay
          centerY={position.y}
          slitHeight={settings.slitHeight}
          maskOpacity={settings.maskOpacity}
          underlayColor={settings.underlayColor}
          underlayOpacity={settings.underlayOpacity}
          slitRect={showSnapSlit ? snapRect : null}
        />
      )}

      {!overlayMode && (
        <header className="hud">
          <h1>Typoscope</h1>
          <p>Move your mouse to position the reading slit.</p>
          <p>
            ↑/↓ nudge · G/H height · [/] opacity · 1 color · T toggle · Shift+A
            auto-snap · D debug
          </p>
          <p>
            Auto-snap: {settings.autoSnap ? "ON" : "off"} · Mode:{" "}
            {settings.snapMode}
            {settings.autoSnap
              ? " · W/L/S/P set mode · M cycle"
              : " (enable Shift+A for W/L/S/P)"}
          </p>
        </header>
      )}

      {overlayMode && showDebug && (
        <div className="debug-panel debug-panel--overlay">
          <p>
            Overlay — D debug · T toggle · Shift+A auto-snap · G/H height · [/]
            opacity
          </p>
          <p>
            Mouse:{" "}
            {mouse ? `${Math.round(mouse.x)}, ${Math.round(mouse.y)}` : "—"}
          </p>
          <p>Slit Y: {Math.round(position.y)} (offset {settings.yOffset}px)</p>
          <p>
            Height: {settings.slitHeight}px · Opacity:{" "}
            {settings.maskOpacity.toFixed(2)}
          </p>
          <p>
            Auto-snap: {settings.autoSnap ? "ON" : "off"} · Mode:{" "}
            {settings.snapMode}
            {settings.autoSnap ? " · W word · L line · S sentence · P paragraph · M cycle" : ""}
          </p>
          {snapRect && (
            <p>
              Snap: {Math.round(snapRect.x)},{Math.round(snapRect.y)}{" "}
              {Math.round(snapRect.width)}×{Math.round(snapRect.height)}
              {snapMeta.source ? ` · ${snapMeta.source}` : ""}
              {snapMeta.latencyMs != null
                ? ` · ${snapMeta.latencyMs.toFixed(1)}ms`
                : ""}
            </p>
          )}
          {snapError && <p>OCR: {snapError}</p>}
          <p>Visible: {settings.visible ? "yes" : "no"}</p>
        </div>
      )}

      {!overlayMode && (
        <>
          <div className="tracking-controls">
            <button type="button" onClick={() => setShowDebug((value) => !value)}>
              {showDebug ? "Hide debug" : "Show debug"}
            </button>
          </div>

          {showDebug && (
            <div className="debug-panel">
              <p>
                Mouse:{" "}
                {mouse ? `${Math.round(mouse.x)}, ${Math.round(mouse.y)}` : "—"}
              </p>
              <p>Slit Y: {Math.round(position.y)} (offset {settings.yOffset}px)</p>
              <p>Height: {settings.slitHeight}px</p>
              <p>Opacity: {settings.maskOpacity.toFixed(2)}</p>
              <p>
                Auto-snap: {settings.autoSnap ? "ON" : "off"} · Mode:{" "}
                {settings.snapMode}
              </p>
            </div>
          )}
        </>
      )}

      {overlayMode && settings.autoSnap && settings.visible && (
        <div className="snap-mode-hud" aria-live="polite">
          {settings.snapMode}
        </div>
      )}
    </div>
  );
}

export default App;
