import type { SlitRect } from "../components/TyposcopeOverlay";
import { useEffect, useRef, useState } from "react";
import { clampBox, type BoxRect } from "../lib/boxGeometry";
import type { Point } from "./useMousePosition";

const FOLLOW_FACTOR = 0.3;

export function useSmoothedPosition(
  target: Point | null,
  boxWidth: number,
  boxHeight: number,
): Point {
  const [position, setPosition] = useState<Point>(() => {
    const initial = clampBox({
      centerX: window.innerWidth / 2,
      centerY: window.innerHeight / 2,
      boxWidth,
      boxHeight,
    });
    return { x: initial.centerX, y: initial.centerY };
  });
  const currentRef = useRef(position);
  const targetRef = useRef(target);
  const boxRef = useRef({ boxWidth, boxHeight });

  useEffect(() => {
    targetRef.current = target;
  }, [target]);

  useEffect(() => {
    boxRef.current = { boxWidth, boxHeight };
  }, [boxWidth, boxHeight]);

  useEffect(() => {
    let frameId = 0;

    const animate = () => {
      const nextTarget = targetRef.current;
      if (nextTarget) {
        const current = currentRef.current;
        const { boxWidth: w, boxHeight: h } = boxRef.current;
        const desired = clampBox({
          centerX: nextTarget.x,
          centerY: nextTarget.y,
          boxWidth: w,
          boxHeight: h,
        } satisfies BoxRect);

        const next = {
          x: current.x + (desired.centerX - current.x) * FOLLOW_FACTOR,
          y: current.y + (desired.centerY - current.y) * FOLLOW_FACTOR,
        };

        currentRef.current = next;
        setPosition(next);
      }

      frameId = window.requestAnimationFrame(animate);
    };

    frameId = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(frameId);
  }, []);

  return position;
}

export function useSmoothedSnapRect(
  target: SlitRect | null,
  /** Higher = snappier. 1 = no lag. */
  factor = 0.72,
): { rect: SlitRect | null; ready: boolean } {
  const [rect, setRect] = useState<SlitRect | null>(null);
  const currentRef = useRef<SlitRect | null>(null);
  const targetRef = useRef(target);

  useEffect(() => {
    targetRef.current = target;
  }, [target]);

  useEffect(() => {
    let frameId = 0;

    const animate = () => {
      const nextTarget = targetRef.current;
      if (nextTarget?.bands?.length) {
        // Line geometry is atomic. Interpolating different sentences or band
        // counts exposes unrelated text between their start/end positions.
        currentRef.current = nextTarget;
        setRect(nextTarget);
      } else if (nextTarget) {
        const current = currentRef.current ?? nextTarget;
        // Track position and size hard so the box follows the unit immediately.
        const posFactor = Math.min(1, factor + 0.08);
        const sizeFactor = Math.min(1, factor);
        const next: SlitRect = {
          x: current.x + (nextTarget.x - current.x) * posFactor,
          y: current.y + (nextTarget.y - current.y) * posFactor,
          width: current.width + (nextTarget.width - current.width) * sizeFactor,
          height:
            current.height + (nextTarget.height - current.height) * sizeFactor,
        };
        // Snap the last millimetre so we don't asymptotically lag.
        if (Math.abs(next.y - nextTarget.y) < 0.35) next.y = nextTarget.y;
        if (Math.abs(next.x - nextTarget.x) < 0.35) next.x = nextTarget.x;
        if (Math.abs(next.width - nextTarget.width) < 0.5) next.width = nextTarget.width;
        if (Math.abs(next.height - nextTarget.height) < 0.5)
          next.height = nextTarget.height;
        currentRef.current = next;
        setRect(next);
      } else {
        currentRef.current = null;
        setRect(null);
      }

      frameId = window.requestAnimationFrame(animate);
    };

    frameId = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(frameId);
  }, [factor]);

  return { rect, ready: rect !== null };
}
