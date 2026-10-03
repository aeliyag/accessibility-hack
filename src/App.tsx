import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import { TyposcopeOverlay } from "./components/TyposcopeOverlay";
import { useGlobalMouse } from "./hooks/useGlobalMouse";
import { useMousePosition } from "./hooks/useMousePosition";
import { useSmoothedPosition } from "./hooks/useSmoothedPosition";
import { useTyposcopeSettings } from "./hooks/useTyposcopeSettings";
import { COLOR_PRESETS } from "./lib/settings";
import { isTauri } from "./lib/isTauri";
import "./App.css";

const MIN_SLIT_HEIGHT = 24;
const MAX_SLIT_HEIGHT = 240;
const SLIT_HEIGHT_STEP = 8;
const OPACITY_STEP = 0.05;
const NUDGE_STEP = 8;

function App() {
  const overlayMode = isTauri();
  const [showDebug, setShowDebug] = useState(false);
  const { settings, loaded, updateSettings } = useTyposcopeSettings();
  const localMouse = useMousePosition();
  const globalMouse = useGlobalMouse(overlayMode && loaded);
  const mouse = overlayMode ? globalMouse : localMouse;
  const position = useSmoothedPosition(
    mouse,
    settings.slitHeight,
    settings.yOffset,
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

  useEffect(() => {
    if (overlayMode) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) {
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

  return (
    <div className={`app ${overlayMode ? "app--overlay" : ""}`}>
      {settings.visible && (
        <TyposcopeOverlay
          centerY={position.y}
          slitHeight={settings.slitHeight}
          maskOpacity={settings.maskOpacity}
          underlayColor={settings.underlayColor}
          underlayOpacity={settings.underlayOpacity}
        />
      )}

      {!overlayMode && (
        <header className="hud">
          <h1>Typoscope</h1>
          <p>Move your mouse to position the reading slit.</p>
          <p>↑/↓ nudge · G/H height · [/] opacity · 1 color · T toggle · D debug</p>
        </header>
      )}

      {overlayMode && showDebug && (
        <div className="debug-panel debug-panel--overlay">
          <p>Overlay mode — D debug · T toggle · G/H height · [/] opacity · 1 color</p>
          <p>
            Mouse:{" "}
            {mouse ? `${Math.round(mouse.x)}, ${Math.round(mouse.y)}` : "—"}
          </p>
          <p>Slit Y: {Math.round(position.y)} (offset {settings.yOffset}px)</p>
          <p>Height: {settings.slitHeight}px · Opacity: {settings.maskOpacity.toFixed(2)}</p>
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
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default App;
