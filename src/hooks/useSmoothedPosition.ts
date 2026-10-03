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
