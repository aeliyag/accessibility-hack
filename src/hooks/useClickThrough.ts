import { invoke } from "@tauri-apps/api/core";
import { useEffect, useSyncExternalStore } from "react";
import {
  getPointerDragCount,
  subscribePointerDragCount,
} from "../lib/pointerDragLock";
import { isTauri } from "../lib/isTauri";

let applied = false;
let pendingTimer = 0;

async function applyClickThrough(ignore: boolean) {
  if (!isTauri()) {
    return;
  }

  if (applied === ignore) {
    return;
  }

  applied = ignore;
  try {
    await invoke("set_click_through", { ignore });
  } catch {
    applied = !ignore;
  }
}

function scheduleClickThrough(ignore: boolean, dragCount: number) {
  window.clearTimeout(pendingTimer);

  if (!ignore || dragCount > 0) {
    pendingTimer = window.setTimeout(() => {
      void applyClickThrough(false);
    }, 0);
    return;
  }

  pendingTimer = window.setTimeout(() => {
    if (getPointerDragCount() > 0) {
      return;
    }
    void applyClickThrough(true);
  }, 200);
}

export function useClickThrough(enabled: boolean) {
  const dragCount = useSyncExternalStore(
    subscribePointerDragCount,
    getPointerDragCount,
    () => 0,
  );

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    scheduleClickThrough(enabled, dragCount);

    return () => {
      window.clearTimeout(pendingTimer);
    };
  }, [enabled, dragCount]);

  useEffect(
    () => () => {
      window.clearTimeout(pendingTimer);
      void applyClickThrough(false);
    },
    [],
  );
}
