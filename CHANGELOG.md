# Changelog

All notable changes to CoreDesk Engine are documented here. This changelog
covers the CoreDesk Engine era; earlier project history lives in git.

## 1.1.0 — 2026-11-07

### The Audio-Reactive Update

- **WASAPI loopback engine** captures the default render endpoint directly and
  follows it when the output device changes. A real FFT pipeline produces
  24 log-spaced bands (60 Hz–16 kHz) with attack/release smoothing, plus level
  and peak-transient detection.
- **0 % CPU when covered**: capture, DSP and rendering suspend entirely while
  the desktop is occluded and resume where they left off.
- **Wallpaper-adaptive colors**: a background probe samples luminance and
  dominant accent under the widget every 2.5 s and on move/resize/style change;
  hues rebase onto the sampled accent with automatic contrast compensation on
  bright backdrops and a fixed fallback palette on grayscale.
- **10 audio visualizer widgets** (v1–v10): Particle Field, Arc Reactor Ring,
  Spectrum Ribbons, Neon Terrain, Resonance Orbs, Bass Bloom, Equalizer
  Skyline, Spiral Sequencer, Comet Trails, Pulse Lattice. Each design keeps its
  own bounds.
- **5 full-screen Live Desktop Effects** rendered natively in Direct2D on the
  WorkerW layer (`le_rain`, `le_grid`, `le_bloom`, `le_aurora`, `le_rings`),
  fed by the same shared spectrum — no WebView2.

### Fixes

- `.cde` theme files no longer carry the global UI language — applying a theme
  never hijacks your locale.
- `WAVEFORMATEXTENSIBLE` formats (the default on most modern endpoints) decode
  correctly.
- Visualizer page load race that could leave the widget hidden.

## 1.0.0 — 2026-11-06

### Renamed — the project is now CoreDesk Engine

- New identity everywhere: binaries (`coredesk.exe`, `coredesk-watchdog.exe`,
  `coredesk-cli.exe`, `coredesk_tap.dll`), Cargo packages (`coredesk`,
  `coredesk-core`, `coredesk-platform`, `coredesk-ipc`, `coredesk-render`,
  `coredesk-watchdog`, `coredesk-cli`, `coredesk-tap`), IPC pipe
  (`\\.\pipe\CoreDeskEngine.<SID>`), single-instance mutex, log file
  (`coredesk.log`), settings window title, installer, MSIX package and every
  user-facing string in all 12 locales. `%APPDATA%\CoreDeskEngine` /
  `%LOCALAPPDATA%\CoreDeskEngine` are the canonical data folders.
- New fox-badge icon (`.ico`, system tray, settings header, installer, MSIX
  Store/tile assets — all generated from `packaging/assets/badge.png`); the
  Settings page was restyled to the dark slate/cybernetic design with cyan
  accents.
- Themes are `.cde` (CoreDesk Engine theme): export, gallery saves and the
  file dialogs all produce it. `.peco` archives import seamlessly — the open
  dialog, `theme import` and double-click accept them, and the library
  migration renames existing `themes/*.peco` entries to `.cde` on first run
  (identical container, same previews and media; `active_theme_path` moves
  with the file). Importing a `.peco` stores it in the library as `.cde`.
- Double-click support: `.cde` is registered under `HKCU\Software\Classes` by
  the Inno installer and via `windows.fileTypeAssociation` in the MSIX
  manifest; `coredesk.exe <file.cde>` imports at cold start or forwards to the
  running instance over the `theme.import` IPC method (`coredesk-cli theme
  import` wraps it).

### Fixes

- **Manual sidecar reverts are honored (ghost-state fix)**: editing
  `Live_Effects_config.json`/`Widget_config.json` back to defaults after a
  tweak had been applied used to be silently undone — the next save rewrote
  the file from stale in-memory values, and the apply merge couldn't see a
  revert at all (a baseline-equal field produces no authored delta). The store
  now keeps a per-entry freshness baseline of the authored deltas it last read
  or wrote, and both the apply-time merge and merge-on-save reconcile with a
  tri-state: an entry whose authored view changed on disk while memory still
  matches the baseline is a hand-edit and wins outright (revert, retune, or
  deletion — the entry is rebuilt from disk, baseline when empty); an entry
  the app touched keeps memory; untouched entries gap-fill as before. Widget
  `bounds`/`adaptiveColor` stay live-mirrored — they can't witness a hand-edit
  and would otherwise mark the active design "app-authored" on every drag.
- **Win+Tab / Task View no longer wedges**: the injected XAML styler was
  matching and writing properties inside the immersive ComposableShell subtree
  (Task View, the Alt+Tab switcher, the input switcher) — the Windhawk themes
  it ports deliberately carry `Experiences.Switcher` targets, so every
  reported switcher element ran matcher ancestor-walks and `SetValue` writes
  on the island's own UI thread while it was still constructing. That stalls
  the island's activation and leaves Task View broken for the whole explorer
  session. Elements whose reported type is under
  `WindowsInternal.ComposableShell.Experiences.` — and all of their
  descendants — are now excluded from matching, dispatch and writes at O(1);
  the marks drop on element removal so recycled handles stay correct, and
  immersive churn no longer counts against the vtw storm limiter. Taskbar,
  Start Menu, Notification Center and File Explorer styling are unaffected.
