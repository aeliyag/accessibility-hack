import { CLOSE_LIMIT_MS, SCREEN_LIMIT_MS } from "../lib/screenBreak";
import type { FaceGuardState } from "../hooks/useFaceGuard";
import "./TimerHud.css";

interface TimerHudProps {
  visible: boolean;
  guard: FaceGuardState;
}

function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function formatCountdown(ms: number): string {
  return String(Math.max(0, Math.ceil(ms / 1000)));
}

function TimerBar({
  label,
  elapsed,
  limit,
  tone,
}: {
  label: string;
  elapsed: number;
  limit: number;
  tone: "close" | "screen" | "alarm";
}) {
  const percent = Math.min(100, (elapsed / limit) * 100);
  return (
    <div className="timer-hud__meter">
      <div className="timer-hud__meter-label">
        <span>{label}</span>
        <span>
          {formatClock(elapsed)} / {formatClock(limit)}
        </span>
      </div>
      <div
        className="timer-hud__track"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={Math.round(limit / 1000)}
        aria-valuenow={Math.round(elapsed / 1000)}
        aria-label={label}
      >
        <div
          className={`timer-hud__fill timer-hud__fill--${tone}`}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}

export function TimerHud({ visible, guard }: TimerHudProps) {
  const breakActive = guard.lookAwayRemainingMs !== null;
  const distance =
    guard.distanceCm === null ? "—" : `${Math.round(guard.distanceCm)} cm`;

  return (
    <>
      {(guard.closeAlarm || breakActive) && (
        <div className="screen-alerts">
          {guard.closeAlarm && (
            <div className="screen-alert screen-alert--close" role="alert">
              Too close to the screen — lean back
            </div>
          )}
          {breakActive && (
            <div className="screen-alert screen-alert--break" role="alert">
              <p>Look at something 20 feet away</p>
              {visible && guard.lookAwayRemainingMs !== null && (
                <p className="screen-alert__timer">
                  {formatCountdown(guard.lookAwayRemainingMs)}
                </p>
              )}
            </div>
          )}
        </div>
      )}

      {visible && (
        <aside className="timer-hud" aria-label="Screen timers">
          <p className="timer-hud__title">Screen timers</p>
          <p className="timer-hud__status">{guard.cameraMessage}</p>
          {guard.camera === "ready" && (
            <>
              <p className="timer-hud__facts">
                Distance {distance}
                {guard.tooClose ? " · too close" : ""}
                {" · "}
                {guard.eyesOnScreen ? "eyes on screen" : "eyes away"}
              </p>
              <TimerBar
                label="Too close"
                elapsed={guard.closeMs}
                limit={CLOSE_LIMIT_MS}
                tone={guard.closeAlarm ? "alarm" : "close"}
              />
              <TimerBar
                label="Eyes on screen"
                elapsed={guard.screenMs}
                limit={SCREEN_LIMIT_MS}
                tone="screen"
              />
              {breakActive && guard.lookAwayRemainingMs !== null && (
                <p className="timer-hud__countdown">
                  Look away {formatCountdown(guard.lookAwayRemainingMs)}s
                </p>
              )}
            </>
          )}
          <p className="timer-hud__hint">Cmd+T hides this panel</p>
        </aside>
      )}
    </>
  );
}
