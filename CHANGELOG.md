# Changelog

All notable changes to CoreDesk Engine are documented here. This changelog
covers the CoreDesk Engine era; earlier project history lives in git.

## 1.3.4 — 2026-11-17

### Shell Hardening

- **Transient XAML targets filtered out of the style wire** — selector
  segments rooted on runtime-transient surfaces (Task View `Switcher.*` tiles
  — the WER-documented Explorer crash module, Snap flyouts, popup menus, OSD
  confirmators, virtual-desktop elements, the hover-flyout window) are dropped
  before delivery. Persistent segments in comma-joined targets survive, so
  tray pills, task-button progress, the Start button and the search gleam keep
  their styling.
- **The Refresh button releases the crash breaker** — the `NoDiag` protection
  previously had no sanctioned exit short of an OS update or manual registry
  edits. Refresh now clears it before restarting Explorer; a repeat crash
  re-trips the lock automatically.
- **Read-only probe on unseen OS builds** — before the first theme write on a
  new build/UBR the resident maps the visual tree without writing a single
  property, keyed per build + UBR + resident image like the breaker.
- **Scan-out GPU matching for video wallpaper** — the D3D11 device is created
  on the adapter that physically drives the primary display (DisplayConfig
  `adapterId`), avoiding cross-GPU frame copies on hybrid laptops and
  multi-GPU desktops; falls back to the default adapter on detection failure.
- **WASAPI loopback retry backoff** — capture failures back off exponentially
  to a 30 s cap instead of retrying every second forever.
- Settings: fence roll-up simplified (five global toggles dropped for fixed
  defaults; auto-fold, count/chevron and tab-switch-on-hover remain) and the
  title-alignment select moved into the Behavior section.

## 1.3.3 — 2026-11-16

### Shell Surfaces & Widgets

- **Notification Center and Quick Settings styling restored** — churn
  suppression muted those hosts mid-burst before styling ever ran; they now
  get a bounded post-quiet resweep while Explorer's throttling stays
  untouched.
- **Start Menu layout no longer scrambles** — `StartMenuExperienceHost` open
  bursts were muting ~2,600 elements before styling; non-tray hosts are exempt
  from suppression and get a resweep once the tree settles.
- **Adaptive widget colors are event-driven** — clock/visualizer widgets no
  longer re-sample their backdrop every 2.5 s; sampling fires only on real
  changes (move/resize/style/wallpaper swaps).
- **Widgets no longer float above maximized windows** — wallpaper
  pause/unpause no longer routes healthy widgets through the topmost band.
- **Video wallpaper startup race closed** — the desktop-layer attach retries
  briefly at startup (~2 s) in both GPU and software paths before falling
  back, instead of dropping to software decode for the whole session.
- **Auto-fold fences** — new opt-in `rollUp.autoFold`: fences expand the
  instant the pointer enters and refold when it leaves, with every existing
  safety latch preserved.

## 1.3.2 — 2026-11-13

### Theme Apply

- **Video wallpaper now switches with the theme** — the recovery poll compares
  the player's path and render config against the configured target, not just
  liveness; transcode swaps are driven immediately; cached variants skip
  redundant transcode passes.
- **New `.cde` imports no longer wipe your fences** — imported themes apply
  visuals only; your fence layout, contents, tabs, loose icons and all fence
  settings are preserved, and clock/visualizer widgets reset to centered
  defaults. Self-saved library themes still apply verbatim, and foreign
  layouts can never silently replace yours.
- **Start Menu styles fit your display scaling** — `Height`/`MaxHeight` rules
  are capped to the real DIP work area at apply time (re-evaluated on live
  scaling changes), so the power button and user tile no longer clip past the
  window edge at 300 % and above.

## 1.3.1 — 2026-11-13

### Shell Stability

- **Explorer crash breaker** — after two exceptional `explorer.exe` exits the
  armed diagnostics session is banned for that OS revision + resident hash
  (`NoDiag` registry keys) and the shell restarts inert; it re-arms
  automatically on an OS update or a rebuilt resident.
- **App crash (`0xc0000005` in `coredesk_tap_*_unloaded`)** — resident
  `ensure_boot` is gated to shell-host processes so hooks firing inside
  `coredesk.exe` can no longer spawn threads that outlive `FreeLibrary`.
- **Dock-style taskbar themes centered** — the engine auto-extends
  `ColumnDefinitions` on taskbar roots that ship without them.
- **8 shipped themes** repaired — a `WindhawkBlur` doubled-bracket typo killed
  their containing ruleset; sticky collapsed taskbar icons handled; theme
  applies no longer dropped on non-UI callback threads.
- Crash-loop recovery no longer clears `taskbarStyle`/`feEffect` while the
  breaker is active; `MODDING_GUIDE.txt` regenerated with the new contracts.

## 1.3.0 — 2026-11-10

### Performance & Multi-Monitor

- **Full-GPU video wallpaper pipeline** — NV12 frames go Media Foundation →
  D3D11 Video Processor → DirectComposition with zero CPU copies per frame;
  high-resolution sources are transcoded once into resolution-capped variants.
- **Eco Mode** — recommended automatically on battery laptops, iGPUs and
  < 2 GiB VRAM: ~30 FPS wallpaper cap, 50 % Live-Effects surface upscaled 2×,
  throttled visualizer rAF, and a separate render-resolution cap picker.
- **Multi-monitor wallpaper control** — per-display offset targeting in span
  and duplicate modes, an Advanced Display Settings page with live canvas
  preview, and blurred edge fill generated from the video's own colors.
- Two new theme packs and a fully rewritten modding manual.
- Measured on Intel Iris Xe @ 2880×1920: app GPU −67 %, RAM working set −50 %,
  private bytes −42 %.

## 1.2.0 — 2026-11-07

### The Modding Update

- **Data-driven style catalog** — every visual is a file in a two-layer
  catalog: factory `{install}\Styles\` shadowed by user
  `%APPDATA%\CoreDeskEngine\Styles\` (file name = id). Six categories:
  Taskbar/Start Menu/Notification Center JSON, Clock/Visualizer HTML, Live
  Effects JSON.
- All 25 clock faces, 10 visualizers and 27 live-effect presets extracted to
  standalone files; widget ids are now their English display names.
- Settings → Modding card: one-click Styles folder, auto-scaffolded category
  folders plus a generated `MODDING_GUIDE.txt`, and (Custom) badges on
  user-layer files.
- Fixes: taskbar/system-tray corruption from adopting a stale resident build's
  identity; visualizer audio freeze when booting into an occluded desktop;
  `g11`–`g15` clocks rejected by the old closed-catalog validator.

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
