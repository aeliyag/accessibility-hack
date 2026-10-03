import {
  AWAY_GRACE_MS,
  CLOSE_LIMIT_MS,
  LOOK_AWAY_MS,
  SCREEN_LIMIT_MS,
  initialScreenBreakState,
  reduceScreenBreak,
  type ScreenBreakState,
  type ScreenObservation,
} from "./screenBreak";

function step(state: ScreenBreakState, ms: number, observation: ScreenObservation) {
  return reduceScreenBreak(state, ms, observation);
}

function assert(condition: boolean, message: string) {
  if (!condition) {
    throw new Error(message);
  }
}

const close: ScreenObservation = { tooClose: true, eyesOnScreen: true };
const reading: ScreenObservation = { tooClose: false, eyesOnScreen: true };
const away: ScreenObservation = { tooClose: false, eyesOnScreen: false };

let state = step(initialScreenBreakState, CLOSE_LIMIT_MS - 1, close);
assert(!state.closeAlarm, "alarm waits for the full close limit");
state = step(state, 1, close);
assert(state.closeAlarm && state.closeMs === CLOSE_LIMIT_MS, "alarm starts at the close limit");

state = step(initialScreenBreakState, CLOSE_LIMIT_MS - 1_000, close);
state = step(state, AWAY_GRACE_MS - 1, reading);
assert(state.closeMs > 0 && !state.closeAlarm, "a short lean-back keeps the close timer");
state = step(state, 1, reading);
assert(state.closeMs === 0 && !state.closeAlarm, "staying back clears the close timer");

state = step(initialScreenBreakState, SCREEN_LIMIT_MS, reading);
assert(state.lookAwayRemainingMs === LOOK_AWAY_MS, "staying on screen starts a 20 second timer");
state = step(state, 1_000, away);
assert(state.lookAwayRemainingMs === LOOK_AWAY_MS - 1_000, "the 20 second timer keeps running");
state = step(state, LOOK_AWAY_MS, reading);
assert(state.lookAwayRemainingMs === null && state.screenMs === 0, "the break resets screen time");

const partialScreenMs = Math.floor(SCREEN_LIMIT_MS / 2);
state = step(initialScreenBreakState, partialScreenMs, reading);
state = step(state, AWAY_GRACE_MS - 1, away);
assert(state.screenMs === partialScreenMs, "a blink does not reset screen time");
state = step(state, 1, away);
assert(state.screenMs === 0, "looking away clears screen time");

console.log("screen break timers ok");
