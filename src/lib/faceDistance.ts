/** Closer than a comfortable laptop reading distance (about 50–70 cm). */
export const TOO_CLOSE_CM = 40;

const REAL_IPD_MM = 63;
const REAL_FACE_WIDTH_MM = 140;
const HORIZONTAL_FOV_RAD = (70 * Math.PI) / 180;

export interface FacePoint {
  x: number;
  y: number;
}

export function median(values: number[]): number | null {
  if (values.length === 0) {
    return null;
  }

  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 0) {
    return (sorted[middle - 1] + sorted[middle]) / 2;
  }
  return sorted[middle];
}

function pixelDistance(
  from: FacePoint,
  to: FacePoint,
  frameWidth: number,
  frameHeight: number,
): number {
  return Math.hypot((from.x - to.x) * frameWidth, (from.y - to.y) * frameHeight);
}

function distanceFromPixels(realMm: number, pixels: number, focalPx: number): number | null {
  if (pixels < 2) {
    return null;
  }
  return (realMm * focalPx) / pixels / 10;
}

/**
 * Estimates face-to-camera distance from iris spacing and cheek width.
 * Laptop webcams are assumed to have about a 70° horizontal field of view.
 */
export function estimateDistanceCm(
  landmarks: FacePoint[],
  frameWidth: number,
  frameHeight: number,
): number | null {
  if (frameWidth < 2 || frameHeight < 2 || landmarks.length < 455) {
    return null;
  }

  const focalPx = frameWidth / 2 / Math.tan(HORIZONTAL_FOV_RAD / 2);
  const estimates: number[] = [];

  if (landmarks.length > 473) {
    const ipdPx = pixelDistance(landmarks[468], landmarks[473], frameWidth, frameHeight);
    const fromEyes = distanceFromPixels(REAL_IPD_MM, ipdPx, focalPx);
    if (fromEyes !== null) {
      estimates.push(fromEyes);
    }
  }

  const facePx = pixelDistance(landmarks[234], landmarks[454], frameWidth, frameHeight);
  const fromFace = distanceFromPixels(REAL_FACE_WIDTH_MM, facePx, focalPx);
  if (fromFace !== null) {
    estimates.push(fromFace);
  }

  if (estimates.length === 0) {
    return null;
  }

  return estimates.reduce((sum, value) => sum + value, 0) / estimates.length;
}

export interface BlendshapeScore {
  categoryName: string;
  score: number;
}

function score(categories: BlendshapeScore[], name: string): number {
  return categories.find((item) => item.categoryName === name)?.score ?? 0;
}

/**
 * True when a detected face is still aimed at the screen.
 * A missing blendshape set counts as on-screen, because the face is in frame.
 */
export function eyesAreOnScreen(categories: BlendshapeScore[] | undefined): boolean {
  if (!categories || categories.length === 0) {
    return true;
  }

  const bothClosed =
    score(categories, "eyeBlinkLeft") > 0.65 && score(categories, "eyeBlinkRight") > 0.65;
  if (bothClosed) {
    return false;
  }

  const side = Math.max(
    score(categories, "eyeLookInLeft"),
    score(categories, "eyeLookInRight"),
    score(categories, "eyeLookOutLeft"),
    score(categories, "eyeLookOutRight"),
  );
  const up = Math.max(score(categories, "eyeLookUpLeft"), score(categories, "eyeLookUpRight"));
  const down = Math.max(
    score(categories, "eyeLookDownLeft"),
    score(categories, "eyeLookDownRight"),
  );

  // The camera sits above the display, so a moderate downward look is still the screen.
  return side < 0.6 && up < 0.48 && down < 0.72;
}
