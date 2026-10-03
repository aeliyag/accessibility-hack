import { load } from "@tauri-apps/plugin-store";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  DEFAULT_SETTINGS,
  STORE_FILE,
  type TyposcopeSettings,
} from "../lib/settings";
import { isTauri } from "../lib/isTauri";

function mergeSettings(partial: Partial<TyposcopeSettings>): TyposcopeSettings {
  return { ...DEFAULT_SETTINGS, ...partial };
}

export function useTyposcopeSettings() {
  const [settings, setSettings] = useState<TyposcopeSettings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(!isTauri());
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;

    void (async () => {
      const store = await load(STORE_FILE);
      const saved = await store.get<Partial<TyposcopeSettings>>("settings");
      if (!cancelled && saved) {
        setSettings(mergeSettings(saved));
      }
      if (!cancelled) {
        setLoaded(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const updateSettings = useCallback(
    (updater: (current: TyposcopeSettings) => TyposcopeSettings) => {
      setSettings((current) => {
        const next = updater(current);

        if (isTauri()) {
          window.clearTimeout(saveTimer.current);
          saveTimer.current = window.setTimeout(() => {
            void (async () => {
              const store = await load(STORE_FILE);
              await store.set("settings", next);
              await store.save();
            })();
          }, 250);
        }

        return next;
      });
    },
    [],
  );

  return { settings, loaded, updateSettings };
}
