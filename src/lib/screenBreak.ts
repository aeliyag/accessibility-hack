export const CLOSE_LIMIT_MS = 10 * 60 * 1000;
export const SCREEN_LIMIT_MS = 20 * 60 * 1000;
export const LOOK_AWAY_MS = 20 * 1000;
/** Blinks and short glances do not reset either timer. */
export const AWAY_GRACE_MS = 3_000;

export interface ScreenObservation {
  tooClose: boolean;
  eyesOnScreen: boolean;
}

export interface ScreenBreakState {
  closeMs: number;
  screenMs: number;
  lookAwayRemainingMs: number | null;
  closeAlarm: boolean;
  awayMs: number;
  farMs: number;
}

export const initialScreenBreakState: ScreenBreakState = {
  closeMs: 0,
  screenMs: 0,
  lookAwayRemainingMs: null,
  closeAlarm: false,
  awayMs: 0,
  farMs: 0,
};

export function reduceScreenBreak(
  state: ScreenBreakState,
  deltaMs: number,
  observation: ScreenObservation,
): ScreenBreakState {
  const delta = Math.max(0, deltaMs);
  const next: ScreenBreakState = { ...state };

  if (observation.tooClose) {
    next.farMs = 0;
    if (!next.closeAlarm) {
      next.closeMs = Math.min(CLOSE_LIMIT_MS, next.closeMs + delta);
      next.closeAlarm = next.closeMs >= CLOSE_LIMIT_MS;
    }
  } else {
    next.farMs += delta;
    if (next.farMs >= AWAY_GRACE_MS) {
      next.closeMs = 0;
      next.closeAlarm = false;
    }
  }

  if (next.lookAwayRemainingMs !== null) {
    next.lookAwayRemainingMs = Math.max(0, next.lookAwayRemainingMs - delta);
    if (next.lookAwayRemainingMs === 0) {
      next.lookAwayRemainingMs = null;
      next.screenMs = 0;
      next.awayMs = 0;
    }
    return next;
  }

  if (observation.eyesOnScreen) {
    next.awayMs = 0;
    next.screenMs = Math.min(SCREEN_LIMIT_MS, next.screenMs + delta);
    if (next.screenMs >= SCREEN_LIMIT_MS) {
      next.lookAwayRemainingMs = LOOK_AWAY_MS;
    }
    return next;
  }

  next.awayMs += delta;
  if (next.awayMs >= AWAY_GRACE_MS) {
    next.screenMs = 0;
  }
  return next;
}
