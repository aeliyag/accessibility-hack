export const COLOR_PRESETS = [
  { underlayColor: "#ffff99", label: "Yellow" },
  { underlayColor: "#cce5ff", label: "Blue" },
  { underlayColor: "#d4edda", label: "Green" },
  { underlayColor: "#f8d7da", label: "Pink" },
  { underlayColor: "#ffffff", label: "White" },
  { underlayColor: "#000000", label: "Black" },
] as const;

export interface TyposcopeSettings {
  slitHeight: number;
  maskOpacity: number;
  underlayColor: string;
  underlayOpacity: number;
  visible: boolean;
  colorPresetIndex: number;
  yOffset: number;
  /** OCR word-snap mode (Shift+A). */
  autoSnap: boolean;
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
};

export const STORE_FILE = "typoscope-settings.json";
