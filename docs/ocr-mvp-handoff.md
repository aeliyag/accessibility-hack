# OCR MVP handoff

## Scope

macOS on the primary display, with word, line, and wrapped-sentence snapping. Paragraph selection is disabled: P does nothing, M cycles the three enabled modes, and stored paragraph preferences migrate to word mode. The backend also maps paragraph requests to word mode.

The change fixes Retina coordinate mapping, capture below the overlay, punctuation-preserving OCR, column-aware wrapped sentences, and separate mask openings per selected line. Pointer misses trigger reacquisition without waiting for cache expiry. Background refresh retains a valid selection through one transient OCR miss, with a bounded stale-cache lifetime. OCR ships as a bundled Swift sidecar rather than depending on a developer build path.

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: 24 geometry, grouping, and refresh tests.
- `npm test`: migration and mode-cycle regression checks.
- `npm run tauri -- build --bundles app`: frontend and native release build, including the macOS OCR sidecar.
- Synthetic Retina OCR covers top, middle, and bottom text; transparent images must fail explicitly.
- Follow the README manual MVP acceptance steps before merge. Automated checks do not establish that the live Times article is flicker-free on the user's machine.

## Integration with main

At review time, main is `6076e91` and includes edit controls, auto-reading, face-distance alarms, and a screen-break timer. This OCR feature branch predates those changes. Integrate both behaviors when resolving overlap; do not replace main's newer UI wholesale with the older OCR UI.

Pay particular attention to:

- `src/App.tsx`: preserve edit/pause/auto-read/timer flows and add OCR state, event filtering, and shortcuts. Shift+M is edit mode on main; plain M cycles OCR only while auto-snap is enabled.
- `src/components/TyposcopeOverlay.tsx` and its CSS: retain main's drag/resize handles while using per-line mask openings for OCR selections.
- Settings migration: preserve main's box, scroll, and panel settings and add the OCR defaults and paragraph-to-word migration.
- Mouse hooks: emit logical screen points once on macOS, without double-dividing by Retina scale. Preserve main's drag-session behavior and keyboard listener/suppressor.
- `src-tauri/src/lib.rs`: register both OCR and main's existing commands in the platform-appropriate invoke handlers.
- `package.json`: preserve main's asset-sync scripts and face-detection dependency as well as the new settings tests.

This handoff prepares the OCR branch for review; it does not claim a completed integration with main, signing/notarization, secondary-display support, or live interaction acceptance.
