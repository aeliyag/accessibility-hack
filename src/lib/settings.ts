export const COLOR_PRESETS = [
  { underlayColor: "#ffff99", label: "Yellow" },
  { underlayColor: "#cce5ff", label: "Blue" },
  { underlayColor: "#d4edda", label: "Green" },
  { underlayColor: "#f8d7da", label: "Pink" },
  { underlayColor: "#ffffff", label: "White" },
  { underlayColor: "#000000", label: "Black" },
] as const;

export type SnapMode = "line" | "word" | "sentence" | "paragraph";

export const SNAP_MODES: SnapMode[] = ["line", "word", "sentence", "paragraph"];

export function cycleSnapMode(mode: SnapMode): SnapMode {
  const i = SNAP_MODES.indexOf(mode);
  return SNAP_MODES[(i + 1) % SNAP_MODES.length];
}

export interface TyposcopeSettings {
  slitHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  visible: boolean;
  colorPresetIndex: number;
  yOffset: number;
  /** OCR auto-snap (Shift+A). */
  autoSnap: boolean;
  /** Snap granularity: line | word | sentence | paragraph */
  snapMode: SnapMode;
}

export const DEFAULT_SETTINGS: TyposcopeSettings = {
  slitHeight: 48,
  maskOpacity: 0.65,
  underlayColor: COLOR_PRESETS[0].underlayColor,
  underlayOpacity: 0.35,
  visible: true,
  colorPresetIndex: 0,
  yOffset: 0,
  autoSnap: false,
  snapMode: "line",
};

export const STORE_FILE = "typoscope-settings.json";
