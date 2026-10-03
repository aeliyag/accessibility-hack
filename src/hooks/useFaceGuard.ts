import { invoke } from "@tauri-apps/api/core";
import { FaceLandmarker, FilesetResolver } from "@mediapipe/tasks-vision";
import { useEffect, useState } from "react";
import {
  playLookAwayChime,
  startCloseAlarm,
  stopCloseAlarm,
  unlockAlertSound,
} from "../lib/alertSound";
import {
  TOO_CLOSE_CM,
  estimateDistanceCm,
  eyesAreOnScreen,
  median,
} from "../lib/faceDistance";
import { isTauri } from "../lib/isTauri";
import {
  initialScreenBreakState,
  reduceScreenBreak,
  type ScreenBreakState,
} from "../lib/screenBreak";

const DETECT_EVERY_MS = 120;
const MAX_FRAME_MS = 2_000;

export interface FaceGuardState {
  camera: "starting" | "ready" | "error";
  cameraMessage: string;
  distanceCm: number | null;
  tooClose: boolean;
  eyesOnScreen: boolean;
  closeMs: number;
  screenMs: number;
  lookAwayRemainingMs: number | null;
  closeAlarm: boolean;
  breakId: number;
}

const initialFaceGuardState: FaceGuardState = {
  camera: "starting",
  cameraMessage: "Starting camera…",
  distanceCm: null,
  tooClose: false,
  eyesOnScreen: false,
  closeMs: 0,
  screenMs: 0,
  lookAwayRemainingMs: null,
  closeAlarm: false,
  breakId: 0,
};

function cameraErrorMessage(error: unknown): string {
  if (error instanceof DOMException && error.name === "NotAllowedError") {
    return "Camera blocked. Allow Typoscope in System Settings → Privacy → Camera.";
  }
  if (error instanceof DOMException && error.name === "NotFoundError") {
    return "No camera found.";
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "Camera could not start.";
}

async function createLandmarker(): Promise<FaceLandmarker> {
  const fileset = await FilesetResolver.forVisionTasks("/mediapipe");
  const options = {
    baseOptions: {
      modelAssetPath: "/models/face_landmarker.task",
      delegate: "GPU" as const,
    },
    runningMode: "VIDEO" as const,
    numFaces: 1,
    outputFaceBlendshapes: true,
  };

  try {
    return await FaceLandmarker.createFromOptions(fileset, options);
  } catch {
    return FaceLandmarker.createFromOptions(fileset, {
      ...options,
      baseOptions: { ...options.baseOptions, delegate: "CPU" },
    });
  }
}

async function withCameraPrompt<T>(task: () => Promise<T>): Promise<T> {
  if (!isTauri()) {
    return task();
  }

  try {
    await invoke("prepare_camera_prompt");
    await new Promise((resolve) => window.setTimeout(resolve, 80));
  } catch {
    // The prompt can still appear if the overlay is already allowed to activate.
  }

  try {
    return await task();
  } finally {
    try {
      await invoke("restore_overlay_policy");
    } catch {
      // Tracking continues even if the overlay cannot return to accessory mode.
    }
  }
}

function publishFrom(
  camera: FaceGuardState["camera"],
  cameraMessage: string,
  breakState: ScreenBreakState,
  distanceCm: number | null,
  tooClose: boolean,
  eyesOnScreen: boolean,
  breakId: number,
): FaceGuardState {
  return {
    camera,
    cameraMessage,
    distanceCm,
    tooClose,
    eyesOnScreen,
    closeMs: breakState.closeMs,
    screenMs: breakState.screenMs,
    lookAwayRemainingMs: breakState.lookAwayRemainingMs,
    closeAlarm: breakState.closeAlarm,
    breakId,
  };
}

export function useFaceGuard(): FaceGuardState {
  const [state, setState] = useState<FaceGuardState>(initialFaceGuardState);

  useEffect(() => {
    const unlock = () => unlockAlertSound();
    window.addEventListener("pointerdown", unlock);
    window.addEventListener("keydown", unlock);
    return () => {
      window.removeEventListener("pointerdown", unlock);
      window.removeEventListener("keydown", unlock);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let frame = 0;
    let stream: MediaStream | null = null;
    let landmarker: FaceLandmarker | null = null;
    const video = document.createElement("video");
    video.className = "face-guard-video";
    video.muted = true;
    video.autoplay = true;
    video.playsInline = true;
    video.setAttribute("playsinline", "true");
    document.body.appendChild(video);

    const stop = () => {
      cancelled = true;
      window.cancelAnimationFrame(frame);
      stream?.getTracks().forEach((track) => track.stop());
      video.pause();
      video.srcObject = null;
      video.remove();
      landmarker?.close();
      stopCloseAlarm();
    };

    void (async () => {
      try {
        landmarker = await createLandmarker();
        if (cancelled) {
          landmarker.close();
          landmarker = null;
          return;
        }

        stream = await withCameraPrompt(() =>
          navigator.mediaDevices.getUserMedia({
            audio: false,
            video: {
              facingMode: "user",
              width: { ideal: 640 },
              height: { ideal: 480 },
            },
          }),
        );
        if (cancelled) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }

        video.srcObject = stream;
        await video.play();
        if (cancelled || !landmarker) {
          return;
        }

        const tracker = landmarker;
        let breakState: ScreenBreakState = { ...initialScreenBreakState };
        let breakId = 0;
        let lastTick = performance.now();
        let lastDetect = 0;
        let publishedKey = "";
        const samples: number[] = [];
        let observation = {
          tooClose: false,
          eyesOnScreen: false,
          distanceCm: null as number | null,
        };

        setState(
          publishFrom("ready", "Looking for a face…", breakState, null, false, false, 0),
        );

        const loop = (now: number) => {
          if (cancelled) {
            return;
          }

          const delta = Math.min(MAX_FRAME_MS, Math.max(0, now - lastTick));
          lastTick = now;

          if (
            now - lastDetect >= DETECT_EVERY_MS &&
            video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA &&
            video.videoWidth > 0
          ) {
            lastDetect = now;
            const result = tracker.detectForVideo(video, now);
            const landmarks = result.faceLandmarks[0];
            if (landmarks) {
              const estimate = estimateDistanceCm(
                landmarks,
                video.videoWidth,
                video.videoHeight,
              );
              if (estimate !== null) {
                samples.push(estimate);
                if (samples.length > 5) {
                  samples.shift();
                }
              }
              const smoothed = median(samples);
              observation = {
                distanceCm: smoothed,
                tooClose: smoothed !== null && smoothed < TOO_CLOSE_CM,
                eyesOnScreen: eyesAreOnScreen(result.faceBlendshapes[0]?.categories),
              };
            } else {
              samples.length = 0;
              observation = { distanceCm: null, tooClose: false, eyesOnScreen: false };
            }
          }

          const next = reduceScreenBreak(breakState, delta, observation);
          if (next.closeAlarm && !breakState.closeAlarm) {
            startCloseAlarm();
          } else if (!next.closeAlarm && breakState.closeAlarm) {
            stopCloseAlarm();
          }
          if (breakState.lookAwayRemainingMs === null && next.lookAwayRemainingMs !== null) {
            breakId += 1;
            playLookAwayChime();
          }
          breakState = next;

          const key = [
            observation.tooClose,
            observation.eyesOnScreen,
            next.closeAlarm,
            next.lookAwayRemainingMs === null ? "off" : "on",
            Math.floor(next.closeMs / 1000),
            Math.floor(next.screenMs / 1000),
            next.lookAwayRemainingMs === null
              ? "x"
              : Math.ceil(next.lookAwayRemainingMs / 1000),
            observation.distanceCm === null ? "x" : Math.round(observation.distanceCm),
          ].join("|");

          if (key !== publishedKey) {
            publishedKey = key;
            setState(
              publishFrom(
                "ready",
                observation.distanceCm === null ? "No face in view" : "Camera on",
                breakState,
                observation.distanceCm,
                observation.tooClose,
                observation.eyesOnScreen,
                breakId,
              ),
            );
          }

          frame = window.requestAnimationFrame(loop);
        };

        frame = window.requestAnimationFrame(loop);
      } catch (error) {
        if (!cancelled) {
          setState({
            ...initialFaceGuardState,
            camera: "error",
            cameraMessage: cameraErrorMessage(error),
          });
        }
      }
    })();

    return stop;
  }, []);

  return state;
}
