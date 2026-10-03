# Typoscope

A cross-platform desktop reading guide that overlays a transparent mask on your screen, leaving a horizontal slit visible so you can focus on one or two lines of text at a time.

Built with **Tauri 2**, **React 19**, and **TypeScript**. The overlay architecture is adapted from the [eye-stick-figure](https://github.com/aeliyag/eye-stick-figure) reference project.

## What is a typoscope?

A typoscope is a classic low-vision reading aid: a card with a cut-out window that blocks surrounding text and glare. This app is the digital version — a system-wide overlay that works on top of browsers, PDF readers, word processors, and more.

It can help people with:

- Low vision or eye strain
- Dyslexia or ADHD-related reading difficulty
- Sensitivity to visual crowding on dense pages

## Features

- Full-screen transparent overlay with a reading slit that follows your mouse
- Click-through overlay — interact with apps underneath normally
- Adjustable slit height, mask opacity, and underlay color
- Global keyboard shortcuts (no focus required)
- Settings persist across restarts
- System tray for show / hide / quit (macOS runs as a background accessory app)

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| ↑ / ↓ | Nudge the reading slit up or down |
| G / H | Increase / decrease slit height |
| [ / ] | Decrease / increase mask opacity |
| 1 | Cycle underlay color presets |
| T | Toggle overlay visibility |
| D | Toggle debug HUD |

## Development

### Prerequisites

- Node.js 18+
- Rust stable (edition 2021)
- macOS: Xcode Command Line Tools (for NSPanel overlay support)

### Install

```bash
npm install
```

### Browser dev mode

Fast UI iteration without rebuilding Rust:

```bash
npm run dev
```

Open http://localhost:1420 — uses local mouse tracking with a dark preview background.

### Tauri dev mode

Full transparent overlay with global mouse and keyboard:

```bash
npm run tauri dev
```

### Production build

```bash
npm run tauri build
```

## Platform notes

| Platform | Status |
| --- | --- |
| **macOS** | Best supported — uses NSPanel for non-activating overlay across Spaces and fullscreen apps |
| **Windows** | Transparent always-on-top window sized to primary monitor |
| **Linux (X11)** | Same as Windows; global mouse polling via `device_query` |
| **Linux (Wayland)** | Limited — global mouse polling and layered overlays are inconsistent on Wayland; arrow-key nudging still works |

Multi-monitor support is limited to the primary monitor in v1.

## Project structure

```
src/
  components/TyposcopeOverlay.tsx   # Mask + reading slit rendering
  hooks/                            # Mouse tracking, smoothing, settings
  lib/                              # Settings defaults and Tauri detection
src-tauri/
  src/overlay_panel.rs              # macOS NSPanel configuration
  src/overlay_window.rs             # Windows/Linux monitor sizing
  src/mouse_hook.rs                 # Global mouse + keyboard polling
  src/tray.rs                       # System tray menu
```

## License

MIT
