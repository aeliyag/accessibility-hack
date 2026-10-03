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

## MVP scope

The current MVP targets macOS on the primary display: free-follow reading, word snapping across the screen, and wrapped sentences within a text column. Paragraph snapping is disabled for this MVP; saved paragraph preferences fall back to word mode. OCR is macOS-only; the basic overlay remains available on other supported platforms.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| ↑ / ↓ | Nudge the reading slit up or down |
| G / H | Increase / decrease slit height |
| [ / ] | Decrease / increase mask opacity |
| 1 | Cycle underlay color presets |
| T | Toggle overlay visibility |
| D | Toggle debug HUD |
| Shift+A | Toggle OCR auto-snap |
| W / L / S | Set snap mode to word / line / sentence (**only while auto-snap is on**) |
| M | Cycle snap modes (**only while auto-snap is on**) |

## Development

### Prerequisites

- Node.js 22.12+ (required by Vite 8)
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
npm run tauri -- dev
```

**macOS permissions**

- **Accessibility** — global hotkeys (including Shift+A)
- **Screen Recording** — OCR auto-snap (Shift+A) captures a region around the cursor

**Shift+A** toggles OCR auto-snap. While it is on, the slit snaps to the unit under the cursor (word / line / sentence). Over blank space, all modes show a stable default-sized box at the cursor (so auto-snap stays visually distinct from free-follow). Use **W / L / S** to pick a mode (or **M** to cycle).

Word coordinates are mapped from captured image pixels into screen points, including Retina displays. OCR captures windows below the overlay; transparent/unreadable captures produce an error instead of silently reusing old word locations. Cached text near the scan anchor is checked for changes while hovering, so scrolling refreshes the selection without unrelated animation elsewhere repeatedly invalidating it.

Entering unmapped text triggers a fresh scan even inside a cached image. Movement can schedule another scan as soon as the current one finishes (at least 100 ms between starts); stationary misses retry 250 ms after completion. A valid selection remains visible during refresh. A single missing OCR result is confirmed with another scan before releasing the selection, with a five-second maximum cache lifetime. Terminal `[typoscope ocr]` messages show scan reasons, raw/accepted word counts, and whether the current pointer matches.

Sentence mode scans the **visible display** for context. Reading order is left to right, then down within the same text column. A horizontal gap greater than approximately **four normal spaces**, estimated from the detected font and word spacing, ends a line's text run instead of joining a sidebar. A line wrap alone does not end a sentence: `.`, `!`, and `?` delimit sentences, with common abbreviations and decimals handled separately. Blank vertical space and first-line indentation separate paragraphs. Text outside the visible screen cannot be included until it is scrolled into view.

Wrapped sentences use an opening for each selected line, preserving the mask over unrelated text before and after the selection. Press **D** to see the active mode, `cache` versus `empty` status, and OCR errors.

### Regression checks

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run build
npm test
```

The Rust tests include actual Vision output from an original synthetic Retina-sized page with paragraphs near the top, middle, and bottom, plus punctuation, wrapped sentence, column-gap, and paragraph-boundary cases.


### Manual MVP acceptance

Use the built-in display and a browser article with wrapped sentences:

- Enable auto-snap with **Shift+A**, then **W**. Check words near the top, middle, and bottom of the display.
- Cross blank space into another paragraph; stop on text and confirm reacquisition after OCR completes.
- Use **S** and hover each line of a wrapped sentence. Only that sentence's line segments should be exposed.
- Leave the cursor on recognized text through multiple refreshes; check for unwanted cancellations.
- Scroll, switch **W/S**, and toggle auto-snap; old geometry should be replaced or released.
- Check tray show/hide/quit and repeat OCR from a copied built app with macOS permissions granted.

Automated geometry and scheduling checks do not replace this live permission and interaction check.

### Production build

```bash
npm run tauri -- build
```

On macOS, `build.rs` compiles the Swift OCR helper for the selected architecture and Tauri bundles it beside the app executable. The generated `src-tauri/binaries/` directory is ignored by Git. The app resolves the helper relative to itself, so OCR does not depend on the original build folder. Local builds are not a signed/notarized public release.

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
