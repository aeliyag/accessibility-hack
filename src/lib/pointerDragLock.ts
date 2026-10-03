type Listener = () => void;

let dragCount = 0;
const listeners = new Set<Listener>();

function notify() {
  listeners.forEach((listener) => listener());
}

export function getPointerDragCount() {
  return dragCount;
}

export function subscribePointerDragCount(listener: Listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function beginPointerDrag() {
  dragCount += 1;
  notify();
  let ended = false;

  return () => {
    if (ended) {
      return;
    }
    ended = true;
    dragCount = Math.max(0, dragCount - 1);
    notify();
  };
}

/** Release any in-flight pointer captures before overlay click-through changes. */
export function releaseActivePointerCaptures() {
  if (typeof document === "undefined") {
    return;
  }

  document.dispatchEvent(
    new PointerEvent("pointerup", {
      bubbles: true,
      cancelable: true,
      pointerId: 1,
      pointerType: "mouse",
    }),
  );
  document.dispatchEvent(
    new PointerEvent("pointercancel", {
      bubbles: true,
      cancelable: true,
      pointerId: 1,
      pointerType: "mouse",
    }),
  );
}
