import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ControlsPanel } from "./components/ControlsPanel";
import { TyposcopeOverlay } from "./components/TyposcopeOverlay";
import { useAutoRead, type AutoReadResult } from "./hooks/useAutoRead";
import { useClickThrough } from "./hooks/useClickThrough";
import { useGlobalMouse } from "./hooks/useGlobalMouse";
import { useMousePosition } from "./hooks/useMousePosition";
import { useTyposcopeSettings } from "./hooks/useTyposcopeSettings";
import { clampBox } from "./lib/boxGeometry";
import { COLOR_PRESETS } from "./lib/settings";
import { isTauri } from "./lib/isTauri";
import { releaseActivePointerCaptures } from "./lib/pointerDragLock";
import type { Point } from "./hooks/useMousePosition";
import "./App.css";

const OPACITY_STEP = 0.05;
const NUDGE_STEP = 8;

function App() {
  const overlayMode = isTauri();
  const [showDebug, setShowDebug] = useState(false);
  const [showControls, setShowControls] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [followPaused, setFollowPaused] = useState(false);
  const [autoScroll, setAutoScroll] = useState(false);
  const [autoReadStatus, setAutoReadStatus] = useState<
    "idle" | "reading" | "no-target"
  >("idle");
  const [frozenCenter, setFrozenCenter] = useState<Point | null>(null);
  const liveCenterRef = useRef<Point>({ x: 0, y: 0 });
  const pausedBeforeEditRef = useRef(false);
  const autoScrollBeforeEditRef = useRef(false);
  const { settings, loaded, updateSettings } = useTyposcopeSettings();
  const localMouse = useMousePosition();
  const globalMouse = useGlobalMouse(overlayMode && loaded);
  const pointer = overlayMode ? globalMouse : localMouse;
  const trackMouse = !editMode && !followPaused;
  const mouse = trackMouse ? pointer : null;

  useClickThrough(overlayMode && loaded && !editMode && !showControls);

  const constrainedSettingsBox = useMemo(
    () =>
      clampBox({
        centerX: settings.centerX,
        centerY: settings.centerY,
        boxWidth: settings.boxWidth,
        boxHeight: settings.boxHeight,
      }),
    [
      settings.boxHeight,
      settings.boxWidth,
      settings.centerX,
      settings.centerY,
    ],
  );

  const followCenter = useMemo(() => {
    if (!mouse) {
      return liveCenterRef.current;
    }

    const clamped = clampBox({
      centerX: mouse.x,
      centerY: mouse.y,
      boxWidth: settings.boxWidth,
      boxHeight: settings.boxHeight,
    });

    return { x: clamped.centerX, y: clamped.centerY };
  }, [mouse, settings.boxWidth, settings.boxHeight]);

  const centerX = editMode
    ? constrainedSettingsBox.centerX
    : followPaused
      ? (frozenCenter?.x ?? constrainedSettingsBox.centerX)
      : autoScroll && pointer
        ? clampBox({
            ...settings,
            centerX: pointer.x,
          }).centerX
        : followCenter.x;

  const centerY = editMode || followPaused || autoScroll
    ? constrainedSettingsBox.centerY
    : followCenter.y;

  liveCenterRef.current = { x: centerX, y: centerY };

  const snapshotToSettings = useCallback(
    (pos: Point) => {
      updateSettings((current) => ({
        ...current,
        ...clampBox({
          ...current,
          centerX: pos.x,
          centerY: pos.y,
        }),
      }));
    },
    [updateSettings],
  );

  const handleAutoReadStatus = useCallback((result: AutoReadResult) => {
    if (!result.scrollable) {
      setAutoReadStatus("no-target");
      return;
    }

    setAutoReadStatus("reading");
  }, []);

  useAutoRead({
    enabled: autoScroll && !editMode && overlayMode,
    linesPerMinute: settings.scrollSpeed,
    onStatus: handleAutoReadStatus,
  });

  const toggleFollowPaused = useCallback(() => {
    if (followPaused) {
      setFollowPaused(false);
      setFrozenCenter(null);
      return;
    }

    const pos = liveCenterRef.current;
    setFrozenCenter(pos);
    snapshotToSettings(pos);
    setFollowPaused(true);
  }, [followPaused, snapshotToSettings]);

  const toggleAutoScroll = useCallback(() => {
    if (autoScroll) {
      setAutoScroll(false);
      setAutoReadStatus("idle");
      return;
    }

    snapshotToSettings(liveCenterRef.current);
    setFollowPaused(false);
    setFrozenCenter(null);
    setAutoReadStatus("idle");
    setAutoScroll(true);
  }, [autoScroll, snapshotToSettings]);

  const finishExitEditMode = useCallback(() => {
    const pos = liveCenterRef.current;
    snapshotToSettings(pos);

    if (pausedBeforeEditRef.current) {
      setFrozenCenter(pos);
      setFollowPaused(true);
    } else {
      setFrozenCenter(null);
      setFollowPaused(false);
    }

    setAutoScroll(autoScrollBeforeEditRef.current);
    setEditMode(false);
  }, [snapshotToSettings]);

  const toggleEditMode = useCallback(() => {
    if (editMode) {
      releaseActivePointerCaptures();
      window.setTimeout(() => {
        finishExitEditMode();
      }, 120);
      return;
    }

    pausedBeforeEditRef.current = followPaused;
    autoScrollBeforeEditRef.current = autoScroll;

    const pos = liveCenterRef.current;
    snapshotToSettings(pos);
    setFollowPaused(false);
    setAutoScroll(false);
    setEditMode(true);
  }, [editMode, autoScroll, finishExitEditMode, followPaused, snapshotToSettings]);

  const handleBoxChange = useCallback(
    (next: {
      centerX: number;
      centerY: number;
      boxWidth: number;
      boxHeight: number;
    }) => {
      updateSettings((current) => ({
        ...current,
        ...clampBox({ ...current, ...next }),
      }));
    },
    [updateSettings],
  );

  const handleScrollSpeedChange = useCallback(
    (scrollSpeed: number) => {
      updateSettings((current) => ({ ...current, scrollSpeed }));
    },
    [updateSettings],
  );

  const handleControlsPanelMove = useCallback(
    (controlsPanelX: number, controlsPanelY: number) => {
      updateSettings((current) => ({ ...current, controlsPanelX, controlsPanelY }));
    },
    [updateSettings],
  );

  const nudgeCenterY = useCallback(
    (delta: number) => {
      let nudgedPoint: Point | null = null;

      updateSettings((current) => {
        const next = clampBox({
          ...current,
          centerY: current.centerY + delta,
        });
        nudgedPoint = { x: next.centerX, y: next.centerY };
        return { ...current, ...next };
      });

      if ((followPaused || autoScroll) && nudgedPoint) {
        setFrozenCenter(nudgedPoint);
      }
    },
    [autoScroll, followPaused, updateSettings],
  );

  const applyControlKey = useCallback(
    (key: string, isRepeat = false) => {
      if (key === "shift+m" && !isRepeat) {
        toggleEditMode();
        return;
      }

      if (key === "shift+x" && !isRepeat) {
        toggleFollowPaused();
        return;
      }

      if (key === "shift+h" && !isRepeat) {
        setShowControls((value) => !value);
        return;
      }

      if (key === "shift+r" && !isRepeat) {
        toggleAutoScroll();
        return;
      }

      if (key === "arrowup") {
        nudgeCenterY(-NUDGE_STEP);
      } else if (key === "arrowdown") {
        nudgeCenterY(NUDGE_STEP);
      } else if (key === "shift+[") {
        updateSettings((current) => ({
          ...current,
          maskOpacity: Math.max(0.1, current.maskOpacity - OPACITY_STEP),
        }));
      } else if (key === "shift+]") {
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
      } else if (key === "d" && !isRepeat) {
        setShowDebug((value) => !value);
      }
    },
    [
      nudgeCenterY,
      toggleAutoScroll,
      toggleEditMode,
      toggleFollowPaused,
      updateSettings,
    ],
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

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      // Tauri shortcuts are delivered by the global Rust listener. Handling
      // them here as well can toggle controls twice while the overlay is
      // accepting pointer input (for example while the help panel is open).
      if (overlayMode) {
        return;
      }

      if (event.shiftKey && event.key.toLowerCase() === "m" && !event.repeat) {
        event.preventDefault();
        toggleEditMode();
        return;
      }

      if (event.shiftKey && event.key.toLowerCase() === "x" && !event.repeat) {
        event.preventDefault();
        toggleFollowPaused();
        return;
      }

      if (event.shiftKey && event.key.toLowerCase() === "h" && !event.repeat) {
        event.preventDefault();
        setShowControls((value) => !value);
        return;
      }

      if (event.shiftKey && event.key.toLowerCase() === "r" && !event.repeat) {
        event.preventDefault();
        toggleAutoScroll();
        return;
      }

      if (event.shiftKey && event.key === "[" && !event.repeat) {
        event.preventDefault();
        applyControlKey("shift+[");
        return;
      }

      if (event.shiftKey && event.key === "]" && !event.repeat) {
        event.preventDefault();
        applyControlKey("shift+]");
        return;
      }

      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      applyControlKey(event.key.toLowerCase(), event.repeat);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    applyControlKey,
    overlayMode,
    toggleAutoScroll,
    toggleEditMode,
    toggleFollowPaused,
  ]);

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

  return (
    <div className={`app ${overlayMode ? "app--overlay" : ""}`}>
      {settings.visible && (
        <TyposcopeOverlay
          centerX={centerX}
          centerY={centerY}
          boxWidth={constrainedSettingsBox.boxWidth}
          boxHeight={constrainedSettingsBox.boxHeight}
          maskOpacity={settings.maskOpacity}
          underlayColor={settings.underlayColor}
          underlayOpacity={settings.underlayOpacity}
          scrollSpeed={settings.scrollSpeed}
          editMode={editMode}
          autoScroll={autoScroll}
          onBoxChange={handleBoxChange}
          onScrollSpeedChange={handleScrollSpeedChange}
        />
      )}

      <ControlsPanel
        visible={showControls}
        x={settings.controlsPanelX}
        y={settings.controlsPanelY}
        onPositionChange={handleControlsPanelMove}
      />

      {followPaused && !editMode && !autoScroll && (
        <div className="follow-paused-badge">
          Follow paused — Shift+X to resume
        </div>
      )}

      {autoScroll && !editMode && autoReadStatus === "no-target" && (
        <div className="follow-paused-badge status-badge--warning">
          Nothing to scroll here — point at a page, document, or list
        </div>
      )}

      {!overlayMode && (
        <header className="hud">
          <h1>Typoscope</h1>
          <p>Move your mouse to position the reading box.</p>
          <p>
            Shift+X pause · Shift+M edit · Shift+R auto-read · Shift+H controls
            · ↑/↓ nudge · Shift+[ / ] opacity · 1 color · D debug
          </p>
        </header>
      )}

      {overlayMode && showDebug && (
        <div className="debug-panel debug-panel--overlay">
          <p>
            Overlay — Shift+X pause · Shift+M edit · Shift+R auto-read · Shift+H help
          </p>
          <p>
            Mouse:{" "}
            {pointer ? `${Math.round(pointer.x)}, ${Math.round(pointer.y)}` : "—"}
          </p>
          <p>
            Box: {Math.round(centerX)}, {Math.round(centerY)} ·{" "}
            {settings.boxWidth}×{settings.boxHeight}
          </p>
          <p>
            Follow: {followPaused ? "paused" : "on"} · Auto-scroll:{" "}
            {autoScroll ? `on (${settings.scrollSpeed} LPM)` : "off"} · Edit:{" "}
            {editMode ? "yes" : "no"} · Opacity:{" "}
            {settings.maskOpacity.toFixed(2)}
          </p>
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
                {pointer ? `${Math.round(pointer.x)}, ${Math.round(pointer.y)}` : "—"}
              </p>
              <p>
                Box: {Math.round(centerX)}, {Math.round(centerY)} ·{" "}
                {settings.boxWidth}×{settings.boxHeight}
              </p>
              <p>
                Auto-read: {autoScroll ? `on (${settings.scrollSpeed} LPM)` : "off"}
              </p>
              <p>Edit: {editMode ? "yes" : "no"}</p>
              <p>Opacity: {settings.maskOpacity.toFixed(2)}</p>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default App;
