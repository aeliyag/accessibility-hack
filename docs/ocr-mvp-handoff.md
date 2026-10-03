# OCR MVP handoff

## Scope

macOS on the primary display, with word, line, and wrapped-sentence snapping. Paragraph selection is disabled: P does nothing, M cycles the three enabled modes, and stored paragraph preferences migrate to word mode. The backend also maps paragraph requests to word mode.

The change fixes Retina coordinate mapping, capture below the overlay, punctuation-preserving OCR, column-aware wrapped sentences, and separate mask openings per selected line. Pointer misses trigger reacquisition without waiting for cache expiry. Background refresh retains a valid selection through one transient OCR miss, with a bounded stale-cache lifetime. OCR ships as a bundled Swift sidecar rather than depending on a developer build path.

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: 26 geometry, grouping, refresh, and shortcut tests.
- `npm test`: five migration, mode-cycle, and timer regression checks.
- `npm run tauri -- build --bundles app`: frontend and native release build, including the macOS OCR sidecar.
- Synthetic Retina OCR covers top, middle, and bottom text; transparent images must fail explicitly.
- Follow the README manual MVP acceptance steps before merge. Automated checks do not establish that the live Times article is flicker-free on the user's machine.

## Integration with main

Merged main `e6d188b`, including edit controls, auto-reading, face-distance alarms, screen-break timers, and speech. The shared canvas renderer keeps main's anti-trail behavior and drag/resize handles while clipping OCR selections to separate line openings.

Settings migration preserves both the box/scroll/panel preferences and OCR preferences. Paragraph mode remains disabled. OCR pauses during edit, paused follow, auto-read scrolling, or a hidden overlay.

Shift+A toggles OCR; W/L/S and M choose modes while it is active. Shift+M remains edit mode, and Shift+S speaks the recognized selection (or captures the ordinary box when no selection exists). One native command handler per platform registers both feature sets, and one macOS keyboard tap dispatches shortcuts.

Live permission, rendering, dragging, scrolling, and speech acceptance still require the README manual checks on macOS. Signing/notarization and secondary-display support remain outside this MVP.
