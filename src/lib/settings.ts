export type SnapMode = "line" | "word" | "sentence";

export const SNAP_MODES: SnapMode[] = ["line", "word", "sentence"];

export function cycleSnapMode(mode: SnapMode): SnapMode {
  const i = SNAP_MODES.indexOf(mode);
  return SNAP_MODES[(i + 1) % SNAP_MODES.length];
}

export const COLOR_PRESETS = [
  { underlayColor: "#ffff99", label: "Yellow" },
  { underlayColor: "#cce5ff", label: "Blue" },
  { underlayColor: "#d4edda", label: "Green" },
  { underlayColor: "#f8d7da", label: "Pink" },
  { underlayColor: "#ffffff", label: "White" },
  { underlayColor: "#000000", label: "Black" },
] as const;

export const MIN_BOX_WIDTH = 120;
export const MIN_BOX_HEIGHT = 24;
export const MAX_BOX_WIDTH_RATIO = 0.95;
export const MAX_BOX_HEIGHT_RATIO = 0.95;

/** Reading speed in lines per minute for auto-read (Shift+R). */
export const MIN_SCROLL_SPEED = 6;
export const MAX_SCROLL_SPEED = 48;
export const DEFAULT_SCROLL_SPEED = 18;

export interface TyposcopeSettings {
  autoSnap: boolean;
  snapMode: SnapMode;
  centerX: number;
  centerY: number;
  boxWidth: number;
  boxHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  visible: boolean;
  colorPresetIndex: number;
  scrollSpeed: number;
  controlsPanelX: number;
  controlsPanelY: number;
}

export interface LegacyTyposcopeSettings extends Partial<Omit<TyposcopeSettings, "snapMode">> {
  snapMode?: string;
  slitHeight?: number;
  yOffset?: number;
}

export function createDefaultSettings(
  viewportWidth = 1440,
  viewportHeight = 900,
): TyposcopeSettings {
  return {
    autoSnap: false,
    snapMode: "line",
    centerX: viewportWidth / 2,
    centerY: viewportHeight / 2,
    boxWidth: Math.min(900, viewportWidth * 0.9),
    boxHeight: 48,
    maskOpacity: 0.65,
    underlayColor: COLOR_PRESETS[0].underlayColor,
    underlayOpacity: 0.35,
    visible: true,
    colorPresetIndex: 0,
    scrollSpeed: DEFAULT_SCROLL_SPEED,
    controlsPanelX: 24,
    controlsPanelY: Math.max(24, viewportHeight - 280),
  };
}

export const DEFAULT_SETTINGS = createDefaultSettings();

export function migrateSettings(raw: LegacyTyposcopeSettings): TyposcopeSettings {
  const viewportWidth =
    typeof window !== "undefined" ? window.innerWidth : 1440;
  const viewportHeight =
    typeof window !== "undefined" ? window.innerHeight : 900;
  const defaults = createDefaultSettings(viewportWidth, viewportHeight);

  const boxHeight = raw.boxHeight ?? raw.slitHeight ?? defaults.boxHeight;
  let centerY = raw.centerY ?? defaults.centerY;
  if (raw.yOffset !== undefined && raw.centerY === undefined) {
    centerY = defaults.centerY + raw.yOffset;
  }

  return {
    autoSnap: raw.autoSnap ?? defaults.autoSnap,
    snapMode: raw.snapMode === undefined ? defaults.snapMode
      : SNAP_MODES.includes(raw.snapMode as SnapMode) ? raw.snapMode as SnapMode : "word",
    centerX: raw.centerX ?? defaults.centerX,
    centerY,
    boxWidth: raw.boxWidth ?? defaults.boxWidth,
    boxHeight,
    maskOpacity: raw.maskOpacity ?? defaults.maskOpacity,
    underlayColor: raw.underlayColor ?? defaults.underlayColor,
    underlayOpacity: raw.underlayOpacity ?? defaults.underlayOpacity,
    visible: raw.visible ?? defaults.visible,
    colorPresetIndex: raw.colorPresetIndex ?? defaults.colorPresetIndex,
    scrollSpeed: normalizeScrollSpeed(raw.scrollSpeed ?? defaults.scrollSpeed),
    controlsPanelX: raw.controlsPanelX ?? defaults.controlsPanelX,
    controlsPanelY: raw.controlsPanelY ?? defaults.controlsPanelY,
  };
}

export const STORE_FILE = "typoscope-settings.json";

function normalizeScrollSpeed(value: number) {
  if (value > MAX_SCROLL_SPEED) {
    return Math.round(
      Math.min(MAX_SCROLL_SPEED, Math.max(MIN_SCROLL_SPEED, value * (18 / 40))),
    );
  }

  return Math.min(MAX_SCROLL_SPEED, Math.max(MIN_SCROLL_SPEED, value));
}

export const mergeSettings = migrateSettings;
