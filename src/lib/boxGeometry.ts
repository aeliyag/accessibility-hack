import {
  MAX_BOX_HEIGHT_RATIO,
  MAX_BOX_WIDTH_RATIO,
  MIN_BOX_HEIGHT,
  MIN_BOX_WIDTH,
} from "./settings";

export interface BoxRect {
  centerX: number;
  centerY: number;
  boxWidth: number;
  boxHeight: number;
}

export function clampBox(rect: BoxRect): BoxRect {
  const maxWidth = window.innerWidth * MAX_BOX_WIDTH_RATIO;
  const maxHeight = window.innerHeight * MAX_BOX_HEIGHT_RATIO;
  const boxWidth = Math.min(maxWidth, Math.max(MIN_BOX_WIDTH, rect.boxWidth));
  const boxHeight = Math.min(
    maxHeight,
    Math.max(MIN_BOX_HEIGHT, rect.boxHeight),
  );

  const halfW = boxWidth / 2;
  const halfH = boxHeight / 2;
  const centerX = Math.min(
    window.innerWidth - halfW,
    Math.max(halfW, rect.centerX),
  );
  const centerY = Math.min(
    window.innerHeight - halfH,
    Math.max(halfH, rect.centerY),
  );

  return { centerX, centerY, boxWidth, boxHeight };
}

export function boxToEdges(rect: BoxRect) {
  const halfW = rect.boxWidth / 2;
  const halfH = rect.boxHeight / 2;

  return {
    left: rect.centerX - halfW,
    top: rect.centerY - halfH,
    right: rect.centerX + halfW,
    bottom: rect.centerY + halfH,
    width: rect.boxWidth,
    height: rect.boxHeight,
  };
}

export type Corner = "nw" | "ne" | "sw" | "se";

export function resizeFromCorner(
  corner: Corner,
  pointerX: number,
  pointerY: number,
  anchor: BoxRect,
): BoxRect {
  const edges = boxToEdges(anchor);
  let left = edges.left;
  let top = edges.top;
  let right = edges.right;
  let bottom = edges.bottom;

  switch (corner) {
    case "nw":
      left = pointerX;
      top = pointerY;
      break;
    case "ne":
      right = pointerX;
      top = pointerY;
      break;
    case "sw":
      left = pointerX;
      bottom = pointerY;
      break;
    case "se":
      right = pointerX;
      bottom = pointerY;
      break;
  }

  const boxWidth = right - left;
  const boxHeight = bottom - top;

  return clampBox({
    centerX: (left + right) / 2,
    centerY: (top + bottom) / 2,
    boxWidth,
    boxHeight,
  });
}
