# Typoscope

A cross-platform desktop reading guide that overlays a transparent mask on your screen, leaving a bounded reading box visible so you can focus on text without visual crowding.

Built with **Tauri 2**, **React 19**, and **TypeScript**. The overlay architecture is adapted from the [eye-stick-figure](https://github.com/aeliyag/eye-stick-figure) reference project.

## What is a typoscope?

A typoscope is a classic low-vision reading aid: a card with a cut-out window that blocks surrounding text and glare. This app is the digital version — a system-wide overlay that works on top of browsers, PDF readers, word processors, and more.

It can help people with:

- Low vision or eye strain
- Dyslexia or ADHD-related reading difficulty
- Sensitivity to visual crowding on dense pages

## Features

- Bounded reading box centered on your mouse (follow mode)
- **Edit mode** (Shift+M toggle) — drag to move, corner handles to resize, scroll-speed slider; no on-screen labels
- **Auto-read** (Shift+R toggle) — slowly scrolls the page or document under your cursor at a comfortable reading speed; the reading box stays put while content moves
- **Controls panel** (Shift+H toggle) — keyboard shortcut reference overlay
- **Screen timers** (Cmd+T toggle) — MediaPipe watches the webcam. An alarm sounds if your face stays too close for 10 minutes. If your eyes stay on the screen for 20 minutes, Typoscope asks you to look 20 feet away and starts a 20 second timer
- Click-through overlay in follow mode — interact with apps underneath normally
- Adjustable box size, mask opacity, and underlay color
- Global keyboard shortcuts (no focus required)
- Settings persist across restarts
- System tray for show / hide / quit (macOS runs as a background accessory app)

## Modes

| Mode | Trigger | Behavior |
| --- | --- | --- |
| **Follow** (default) | — | Box center tracks mouse; clicks pass through to apps below |
| **Edit** | Shift+M (toggle) | Box position frozen; drag body or corners to arrange; blue slider adjusts scroll speed; overlay captures mouse |
| **Auto-read** | Shift+R (toggle) | Scrolls content under the cursor at reading speed; box X tracks mouse, Y stays fixed |

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| Shift+X | Toggle mouse follow (static / dynamic) |
| Shift+M | Toggle edit mode (drag / resize / scroll speed) |
| Shift+R | Toggle auto-read (scroll content under cursor) |
| Shift+H | Show / hide controls panel |
| Cmd+T | Show / hide screen timers |
| ↑ / ↓ | Nudge the reading box up or down |
| Shift+[ / Shift+] | Decrease / increase mask opacity |
| 1 | Cycle underlay color presets |
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

Open http://localhost:1420 — uses local mouse tracking with a dark preview background. Shift+M works via keyboard.

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
| **macOS** | Best supported — uses NSPanel for non-activating overlay across Spaces and fullscreen apps. Grant **Accessibility** so shortcuts (Shift+X, Cmd+T, etc.) do not type into apps below, and **Camera** so face distance and the 20-minute eye break can run |
| **Windows** | Transparent always-on-top window sized to primary monitor |
| **Linux (X11)** | Same as Windows; global mouse polling via `device_query` |
| **Linux (Wayland)** | Limited — global mouse polling and layered overlays are inconsistent on Wayland; arrow-key nudging still works |

Multi-monitor support is limited to the primary monitor in v1.

## Project structure

```
src/
  components/TyposcopeOverlay.tsx   # Mask + reading box rendering
  components/TyposcopeHandles.tsx   # Edit mode drag + corner resize
  components/ScrollSpeedHandle.tsx  # Edit mode scroll-speed slider
  components/ControlsPanel.tsx      # Shift+H shortcut reference
  components/TimerHud.tsx           # Cmd+T screen timers and break alerts
  hooks/                            # Mouse tracking, face guard, settings, click-through
  lib/                              # Settings, box geometry, screen-break timers
src-tauri/
  src/overlay_panel.rs              # macOS NSPanel configuration
  src/overlay_window.rs             # Windows/Linux monitor sizing
  src/mouse_hook.rs                 # Global mouse + keyboard polling
  src/tray.rs                       # System tray menu
```

## License

MIT
