import { useEffect, useRef, useState } from "react";
import type { Point } from "./useMousePosition";
import type { SlitRect } from "../components/TyposcopeOverlay";

export function useSmoothedSnapRect(
  target: SlitRect | null,
  factor = 0.18,
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
      if (nextTarget) {
        const current = currentRef.current ?? nextTarget;
        const next: SlitRect = {
          x: current.x + (nextTarget.x - current.x) * factor,
          y: current.y + (nextTarget.y - current.y) * factor,
          width: current.width + (nextTarget.width - current.width) * factor,
          height:
            current.height + (nextTarget.height - current.height) * factor,
        };
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

export function useSmoothedPosition(
  target: Point | null,
  slitHeight: number,
  yOffset: number,
  factor = 0.12,
): Point {
  const [position, setPosition] = useState<Point>(() => ({
    x: window.innerWidth / 2,
    y: clampY(window.innerHeight / 2, slitHeight),
  }));
  const currentRef = useRef(position);
  const targetRef = useRef(target);
  const yOffsetRef = useRef(yOffset);

  useEffect(() => {
    targetRef.current = target;
  }, [target]);

  useEffect(() => {
    yOffsetRef.current = yOffset;
  }, [yOffset]);

  useEffect(() => {
    let frameId = 0;

    const animate = () => {
      const nextTarget = targetRef.current;
      if (nextTarget) {
        const current = currentRef.current;
        const desiredY = clampY(
          nextTarget.y + yOffsetRef.current,
          slitHeight,
        );
        const next = {
          x: current.x,
          y: current.y + (desiredY - current.y) * factor,
        };

        currentRef.current = next;
        setPosition(next);
      }

      frameId = window.requestAnimationFrame(animate);
    };

    frameId = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(frameId);
  }, [factor, slitHeight]);

  return position;
}

function clampY(y: number, slitHeight: number): number {
  const half = slitHeight / 2;
  const minY = half;
  const maxY = Math.max(half, window.innerHeight - half);
  return Math.min(maxY, Math.max(minY, y));
}
