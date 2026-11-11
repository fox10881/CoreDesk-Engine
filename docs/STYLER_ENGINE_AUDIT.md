# Styler Engine Audit Log

Audit of the four style paths in `coredesk-tap` (see `crates/tap/src/`):

| UI option | Wire kind | Host process | Entry |
|---|---|---|---|
| Taskbar style | `CDES-TBS1` | `explorer.exe` | `styler.rs` |
| Start Menu style | `CDES-SMS1` | `StartMenuExperienceHost.exe` | `styler.rs` |
| Notification Center style | `CDES-NCS1` | `ShellExperienceHost.exe` / `ShellHost.exe` | `styler.rs` |
| Explorer backdrop | — (named pipe `ebm.rs`) | `explorer.exe` (cabinet windows) | `ebm.rs` |

Live-inspection tool: **UWPSpy** — inject `UWPSpy.dll` into the host pid,
find the `UWPSpy - PID: <n>` window, walk `SysTreeView32`.

---

## Lessons learned (accumulate here)

1. **Taskbar and SystemTray are separate XAML islands.** `Shell_TrayWnd`
   hosts `Taskbar.TaskbarFrame` and `SystemTray.SystemTrayFrame` on
   different islands; cross-island parents are reported as opaque bridge
   handles, so ancestor whitelists must whitelist roots by type prefix
   (`Taskbar.`, `SystemTray.`, `SearchUx.`), not only by parent chain.

2. **Root-of-island elements have no XAML parent.** `GetParent` fails on
   island roots — scope checks need an explicit "unattached root allowed
   when its own type is in-prefix" exception.

3. **`Grid.Column=N` is inert on a column-less Grid.** Writing the attached
   property works, but a Grid without `ColumnDefinitions` stacks all
   children in cell 0 — only `HorizontalAlignment` then decides position.
   Engine fix: `ensure_grid_columns` builds `[*,auto…,*]` columns, claims
   are refcounted in `ElemState::grid_cols` and released on unstyle.

4. **`ColumnDefinitions` is not a DependencyProperty.** A theme's
   `ColumnDefinitions:=<…>` setter parses fine but `SetValue` fails
   silently — the engine's auto-column path is what actually creates them.

5. **`TaskbarCollapsibleLayout` collapse is sticky (hysteresis).** Once the
   MUX layout sees a narrow measure it pins buttons to ~30px slots until a
   wider arrange un-sticks them. A `MinWidth` floor on `TaskbarFrame`
   (~items*55+start) breaks the equilibrium.

6. **MUX (WinUI3) islands inside the taskbar.** `TaskbarFrameRepeater` is
   a WinUI3 `ItemsRepeater` hosted in an island — its child bounds come
   from `*AutomationPeer`s via UIA, not wux properties.

7. **`hung=False` + dump count is the stability bar** for every resident
   change; `Shell_TrayWnd` responsiveness check + N drag/Snap-Assist
   cycles before calling any tap change safe.

8. **churn mute ≠ dropped.** Elements arriving during churn storms are
   muted *before* `style_element` — always check `SCOPE drop`/`churn`
   counters first when "a style didn't apply".

9. **`props=N` in `styled` lines counts *applied* setters.** `props=5`
   for `SystemTrayFrame` means `Grid.Column=2` + `Height=80` + `Width`
   + `HorizontalAlignment` + `Margin` all wrote OK.

10. **Theme re-apply mutates live shells without restarting Explorer** —
    `coredesk-cli settings set taskbarStyle <name>` round-trips through
    `CleanSlate` to force a real re-apply when the value is unchanged.

---

## Architecture map (from `lib.rs`/`styler.rs`/`command.rs`/`site.rs`)

- ONE engine body (`styler_engine.rs`) is `include!`d twice: `wux`
  (Windows.UI.Xaml — taskbar/start/NC) and `mux` (Microsoft.UI.Xaml —
  File Explorer `CDES-FES1`). Differences per instantiation: dispatcher
  type, type namespaces, and `blur_brush` (wux = real compositor blur;
  mux = translucent solid, since in-app acrylic does nothing in islands).
- Injection: `WH_CALLWNDPROC` (`CallWndProc`) for pumping hosts,
  `WINEVENT_INCONTEXT` (`WinEventProc`) for suspended AppContainer hosts;
  both retry `ensure_boot` — the resident self-PINs in `DllMain`
  (GetModuleHandleEx PIN) or it would unload ~40ms after inject.
- Command surface: message-only window `CoreDeskTAP` / `CoreDeskTAPSM` /
  `CoreDeskTAPNC`; wires arrive as UTF-8 `WM_COPYDATA`. `EBML`/`EBMU`
  are separate 4cc markers for the EBM relay. `build_prop_name` stamps a
  protocol build number on the window.
- Wire format: magic line (`CDES-…`), `L layout`, `C` classic marker,
  `R k=v` resvars, `T target` rulesets, `S style` lines, `W css` lines,
  `E` end. `T` splits on un-quoted commas; styles after `T` apply to all
  pending rulesets.
- Sessions: `site::install` (wux via `Windows.UI.Xaml.dll` diagnostics)
  + `site::install_winui3` (mux via `FrameworkUdk` from SystemApps).
  **Shell pids never keep the mux session** — `ensure_watching_winui3`
  actively unadvises it when `is_shell_tray_host()` (the proven
  Switcher-crash path).

## Safety gates (all five, in `on_element` order)

1. `is_unloaded` — teardown gate.
2. Dispatcher capture BEFORE any gate (a muted element still proves a
   live UI thread; starving this left `pending` themes unapplied).
3. `churn_muted` — ≥25 events/100ms mutes ALL element work until 400ms
   quiet; READVISING replay exempt.
4. `is_immersive` — `WindowsInternal.ComposableShell.Experiences.*` and
   `SnapLayout.` subtrees marked root→descendants, released untouched.
5. `in_taskbar_scope` — **shell-pid only** (`is_shell_tray_host()`);
   SMEH/NCH/SearchHost keep their full element surface.
6. `storm_limited` — >600 events/s pauses styling, clears below 60/s.
7. Write-time gates in `style_element`/deferred closure: `known()` liveness
   + `VisualTreeHelper::GetParent` attachment (zombie check) +
   `churn_recently_hot` (includes `foreign_island_active` + 500ms grace).

Write rules: weak refs only across deferral; `SetValue` never inside the
`OnVisualTreeChange` callback itself; mutexes never held across COM calls;
every dispatched closure is panic+HRESULT airlocked (stowed 0xc000027b
kills the host).

## Per-path findings

### 1. Taskbar (`CDES-TBS1`, explorer.exe)

- `is_shell_tray_host()` = `owns_shell_tray()` (`Shell_TrayWnd` in this
  pid) with a self-healing recheck while the flag is false.
- Full whitelist of `TASKBAR_SCOPE_ROOTS` = `Taskbar.`/`SystemTray.`/
  `SearchUx.` — SystemTray/SearchUx are separate islands whose parents
  are invisible bridge handles (this session's fix).
- `ensure_grid_columns` synthesises `[*,auto…,*]` columns when a theme
  writes `Grid.Column=N` beyond the parent's definitions; refcounted
  (`ElemState::grid_cols` + `release_grid_columns`).
- `frame_diag` logs parent chain/columns/children for the two frame types.
- Verified live this session: `[*,auto,auto,*]` grid, `props=5/8/9`
  applied to frame/tray/grid elements, `hung=False`, 10+ drag cycles
  clean.

### 2. Start Menu (`CDES-SMS1`, StartMenuExperienceHost.exe + SearchHost.exe)

- Same engine, NO scope whitelist (full surface styled).
- `L forceNewLayout|newLayoutSideBySide` → `arm_layout_detour` hooks
  `ntdll!RtlQueryFeatureConfiguration` forcing feature ids
  48697323/47205210/49221331/49402389; `throttled_exit` writes
  `%TEMP%\cde_smeh_exit.tick` (10s throttle) + `ExitProcess` so SMEH
  rebuilds under forced flags; `prearm_smeh_layout_from_stamp` re-arms
  the detour on respawn within 60s. Classic variant rules (`C` section)
  chosen when `!is_redesigned`.
- `SearchHost.exe` gets the same wire under `CoreDeskTAPSM` but ONLY the
  `W` CSS matters (webview search results); the detour+exit are gated on
  `is_smeh()` so SearchHost never bounces.
- `webview.rs`: `AddScriptToExecuteOnDocumentCreated` injection into
  `HostedWebView2Control`/`*.WebView2` elements; `web_pending` retries a
  not-ready `CoreWebView2` for ≤20s/512 attempts; `web_injected` keeps
  script ids for clean uninject; `session_reset` clears all three maps.

### 3. Notification Center (`CDES-NCS1`, ShellExperienceHost.exe / ShellHost.exe)

- `is_nch()` accepts BOTH process names (Action Center moved between
  hosts across builds).
- Full element surface (no scope gate). Multi-island host: toast flyout,
  focus session, NC panel run on different dispatchers —
  `MARSHAL_ON_ELEMENT` writes each element on its own
  `DependencyObject::Dispatcher` (RPC_E_WRONG_THREAD otherwise).
- Rulesets that fail `XamlReader.Load` at apply time (assembly not yet
  loaded — `ActionCenter.*`, `QuickActions.*`) stay `failed` and are
  lazily re-resolved by `retry_failed_rules` when an element of that
  type first reports (`RETRY_LIMIT` bounded, per-thread TL cache).

### 4. Explorer backdrop (`ebm.rs` + mux `CDES-FES1`)

- `EBML` wire = UTF-16 path; `validate_ebm_path` demands
  `%LOCALAPPDATA%\CoreDeskEngine\residents\ebm-<16hex>\ExplorerBlurMica.dll`,
  absolute, no traversal, single trailing NUL — then plain
  `LoadLibraryW` in-process (no COM, no admin); `EBMU`/`shutdown` →
  `FreeLibrary` (EBM's own detach restores windows). `EBM_MODULE`
  handle is the double-load guard. Unit-tested.
- `CDES-FES1` styles File Explorer's WinUI3 islands through the mux
  engine: same rules engine, `DispatcherQueue` marshalling, solid-tint
  fallback brush for `blur()` (see `styler.rs` comment — DWM accent
  blurs the backdrop; islands only need translucency, alpha clamped
  0x10..=0x28/0x45).

## Restore / teardown semantics

- `restore_all` takes `elements` out of the registry (no re-entry),
  restores each on ITS OWN dispatcher (`MARSHAL_ON_ELEMENT`), unmerges
  the theme dictionary, resyncs webviews. `original`/`ClearValue` per
  slot; unregisters ALWAYS run, value-restores only when the element is
  still `GetParent`-attached (zombie-safe).
- `session_reset` (new session/island): drops elements+marks but keeps
  armed wire — the re-advise replay restyles the fresh tree.
- `on_element_removed` NEVER skipped (stale handles recycle into
  SnapLayout); during teardown-hot it `mem::forget`s state rather than
  drop revokers on a tree-locked thread (the AppHang lesson).
- `queue_release`→`drain_releases`: 150ms-delayed worker, waits out
  churn, `UnregisterInstance` (undoc `IXamlDiagnosticsTestHooks` IID)
  only for stateless handles; `GetIInspectableFromHandle` on dead
  handles banned outright.
- `shutdown`: restore on dispatcher + 3s event wait, then wipe all state
  + disarm detour.

## Known gaps / edge cases

- `restore_all` early-returns while `churn_recently_hot` with NO requeue:
  a `reset()` landing mid-storm leaves visuals applied (`applied=None`,
  elements still styled) until the next theme apply re-runs the sweep.
  Low impact (element removes still restore individually) but real.
- `MinWidth` floor for `TaskbarCollapsibleLayout` hysteresis is a
  THEME-side fix; an engine-side `Measure(∞)` assist was considered and
  deferred (unverifiable + risky for marginal gain — the user-level
  MinWidth covers it).
- `in_taskbar_scope` trusts reported-type prefixes: a theme could in
  principle mark foreign islands by *type* — but only handles inside
  marked subtrees get written anyway; generic `Grid`/`Border` under
  foreign roots stay blocked via parent marks.
- NCS/SMS wires share the wux session in multi-island hosts — a theme
  authored for one island could match same-named elements on sibling
  islands; parent matchers are the tool to disambiguate.

## Live verification (2026-11-11, build `v9_60d068…`)

Resident present in ALL five hosts: `explorer` (tap + `ExplorerBlurMica.dll`),
`StartMenuExperienceHost`, `ShellExperienceHost`, `ShellHost`,
`SearchHost` — all `coredesk_tap_v9_60d068303fa3442f.dll`.

Wire traffic observed in `%TEMP%\coredesk_tap.log`:

- `Taskbar theme … modern=17/39/62/93` — applies cleanly.
- `NotificationCenter theme … modern=72/84/151 resvars=19` — SEH+ShellHost
  both wired.
- `StartMenu theme layout=ForceNew modern=71` — the detour+ExitProcess
  path exercised in production (forced-layout theme selected; SMEH
  exited, respawned pre-armed, and now runs `Default` wires fine).
- No `FileExplorer` (FES1) wires — the mux File-Explorer theme path
  exists but is unused by current styles; EBM covers the backdrop
  option on this install.

**FINDING — live bug, FIXED**: `lazy resolve
"Windows.UI.Xaml.Shapes.Rectangle" still failing` — root cause was a
systematic typo in 8 factory themes + the user copy:
`Fill:=<<WindhawkBlur …/>` (double `<<`). `parse_blur` requires the
`<WindhawkBlur ` prefix → falls through to `<Setter.Value>` XAML →
`XamlReader.Load` always rejects malformed markup → the whole
`Rectangle#BackgroundStroke@HoverFlyoutBackgroundControl` ruleset was
permanently dead (hover-flyout backdrop never styled). Fixed:
`<<WindhawkBlur`→`<WindhawkBlur` in all OS26 variants, One_UI_8_5 Dock +
Taskbar, and the user's ClearMacDock copy. Same-family duplicate `Fill:=`
ordering preserved (last wins = BlurAmount 8, as the theme intended).

## Mistakes made during this audit & fixes

- Assumed RTL from the tray being left — `FlowDirection=0` proved LTR;
  the real cause was the column-less parent Grid. Lesson: read
  `FlowDirection` + `grid_cols` from `frame_diag` before theorising.
- `MinWidth=500` "fix" stretched `TaskbarFrame` under the tray region —
  UIA bounds showed the overlap; reverted to `:=100` (collapse was
  already un-stuck). Lesson: fix the *state*, not the symptom — and
  verify bounds against neighbours, not just the target.
- `props=1` lines in the log were mistaken for an old resident — they
  were `ShellExperienceHost`'s Control-Center elements styled by the SAME
  build (rules differ per type). Lesson: correlate by build-stamp
  (`v9_…`), not by prop counts.
- UWPSpy automation (corrected after real runs): `BM_CLICK` on the Spy
  button is what actually works — the earlier claim that it's unreliable
  was wrong. What really fails is the *real-mouse* click: it lands on
  whatever window is physically topmost at those coords (the picker
  dialog buried under the IDE ate the click silently). Lessons:
  (a) prefer `BM_CLICK` — no foreground/Z-order dependency;
  (b) if a real click is ever needed, raise `TOPMOST` first AND verify
    the dialog `IsWindowVisible` + owns the coords;
  (c) the process list refreshes live — re-select the row immediately
    before clicking, or the selection points at a different process;
  (d) `LVITEMW` on x64: `pszText` at offset **24**, `cchTextMax` at **32**
    (8-byte alignment — offset 20 silently returns empty strings);
  (e) PowerShell `[void]x | Out-Null` throws
    "Argument type cannot be System.Void" — pipe OR cast, never both.

## UWPSpy live verification (2026-11-11, genuine session)

Injection via picker-dialog row-select + `BM_CLICK` — verified by
`Get-Process <pid> -Modules` containing `UWPSpy.dll`:

| Host | PID | Inject | Live tree captured |
|---|---|---|---|
| `explorer.exe` | 15024 | OK | `DesktopWindowXamlSource → PopupRoot → RootScrollViewer → ScrollContentPresenter → Border → Grid → Taskbar.TaskbarFrame(TaskbarFrame) → {Grid RootGrid, Taskbar.TaskbarBackground(BackgroundControl) → {Rectangle BackgroundFill, Rectangle BackgroundStroke}, Microsoft.UI.Xaml.Controls.ItemsRepeater(TaskbarFrameRepeater) → {Taskbar.ExperienceToggleButton(LaunchListButton), Taskbar.TaskListButtonPanel(ExperienceToggleButton) → {Grid → Border BackgroundElement, AnimatedVisualPlayer, SpriteVisual, ContainerVisual×2}}}` |
| `StartMenuExperienceHost.exe` | 10932 | OK | `Windows.UI.Xaml.Window(Start) → PopupRoot / FullWindowMediaRoot / RootScrollViewer → ScrollContentPresenter → Border → StartMenu.StartBlendedFlexFrame → Grid FrameRoot → Grid AnimationRoot → {Rectangle MaxHeightEnforcer, Border StartDropShadow, Border DropShadowDismiss, Grid MainMenu → {Border AcrylicBorder, Grid MainContent → Grid → StartMenu.SearchBoxToggleButton(SearchBoxToggle) → Grid → {Border BorderElement, Grid UnderlineContainer, Border BorderUnderline}}}` |
| `ShellHost.exe` | 8884 | OK | `DesktopWindowXamlSource → PopupRoot → RootScrollViewer → ScrollContentPresenter → Border → ControlCenter.ControlCenterPage → Grid RootGrid → Grid RootContent → ControlCenter.MediaTransportControls → …` — same surface for `Win+A` (quick settings) and `Win+N` (notification center + calendar + Focus). |
| `SearchHost.exe` | 12208 | OK | `Windows.UI.Xaml.Window(Search) → PopupRoot / FullWindowMediaRoot / RootScrollViewer → ScrollContentPresenter → Border → Cortana.UI.Views.TaskbarSearchPage → Grid RootGrid → Grid OuterBorderGrid → Grid BorderGrid → {Border AppBorder, AccentAppBorder, LayerBorder, AccentLayerBorder, dropshadow, TaskbarMargin, Button StartMenuVirtualPrevious(?), Border TaskbarSearchBackground, Cortana.UI.Views.RichSearchBoxControl(SearchBoxControl) → Grid RootGrid → CortanaRichSearchBox(SearchTextBox)}` |
| `ShellExperienceHost.exe` | 6908 | **refused** | injection failed — row selected + `BM_CLICK` fired (same path that worked for 3 other hosts), no `UWPSpy.dll` in modules, picker stayed open. Host-level restriction (legacy Action-Center host). Modern NC is in `ShellHost` anyway — coverage preserved. |

**Key mechanics learned (why automation kept failing):**
- Injected `UWPSpy.dll` creates its `UWPSpy - PID: <pid>` tree window
  **lazily — only once a live XAML surface exists** in the target. For
  SMEH/ShellHost/SearchHost the window appears only while the surface
  (Start / QS·NC / Search) is open; it dies with it.
- Those windows live on an immersive desktop invisible to
  `EnumWindows`, `FindWindowEx`, `UIAutomation` root children and
  `FromPoint` (all return nothing/other windows). Screenshots capture
  them fine; real mouse input reaches them; programmatic `TVM_*`
  walking cannot (different desktop — `SysTreeView32` is unreachable).
- Any real click on the spy window steals focus → the shell surface
  closes → the spy window dies. Live extraction is therefore
  screenshot-based; scroll/expand interaction is impractical.
- `%TEMP%\coredesk_tap.log` is the resident-side apply log (debug
  builds append; concurrent multi-host writes interleave and corrupt
  `Instant { t: }` stamps — read past the noise).

**Cross-check vs the engine (`coredesk_tap.log`, same session):**
- `NotificationCenter theme modern=72` → live `ControlCenter.*`
  elements genuinely styled: `ControlCenter.PaginatedToggleButton`
  `props=5`, `ControlCenter.AsyncSlider` `props=1`,
  `Grid#ControlCenterRegion` `matched` — matches the ControlCenter
  tree above.
- `resolve failed for "ActionCenter.FlexibleItemView" /
  .FlexibleToastView / .NotificationListViewItem` — the theme's
  legacy-surface rules die at type resolution (assembly not loaded in
  the host). Confirmed the theme dual-targets: 39 `ControlCenter.*` +
  19 `ActionCenter.*` rules — the dead half degrades safely; not a bug.
- `resolve failed for "Cortana.UI.Views.RichSearchBoxControl"` — the
  same type IS live in SearchHost's own tree (UWPSpy screenshot), so
  this failure is a **wrong-host resolve** (the ruleset ran in a host
  lacking `Cortana.UI` assemblies), proving resolves are per-process.
- `StartMenu theme modern=100` — matches the `StartMenu.*` tree above.


## Resource study — feEffect backdrop materials (dwm, measured live)

Mechanism map (`EBM_CHANGES_SUMMARY.md` — EBM `effect=` + DWM calls):

| Setting | effect= | DWM mechanism | Per-frame recompute? |
|---|---|---|---|
| `blur` | 0 | `ACCENT_BLURBEHIND` | yes — live blur of whatever is behind |
| `acrylic` | 1 | `ACCENT_ACRYLIC` | yes — blur + noise + tint composite |
| `mica` | 2 | `DWMSBT_MAINWINDOW` | no — wallpaper sampled once, static texture |
| `blurclear` | 3 | `ACCENT_ACRYLIC_NO_HOSTBK` | yes — acrylic pipeline w/o host backdrop |
| `micaalt` | 4 | `DWMSBT_TABBEDWINDOW` | no — wallpaper sample, alt tint |

Measured `dwm.exe` GPU-engine utilisation (Get-Counter
`\GPU Engine(pid_<dwm>*_engtype_3D)\Utilization Percentage`),
File-Explorer window present, two passes:

| Effect | Static (mpv animating behind) | Busy (window dragged + mpv behind) |
|---|---|---|
| WindowsDefault | 11.78% | 9.55% |
| `blur` | 14.34% | 13.40% |
| `acrylic` | 14.41% | 13.40% |
| `mica` | 10.25% | 9.53% |
| `blurclear` | 14.36% | (see notes) |
| `micaalt` | 10.26% | (see notes) |

Key finding — the cost is conditional, not constant:

- Live-blur family (`blur`/`acrylic`/`blurclear`) costs ≈ **+3-4%
  dwm GPU only while content BEHIND the window animates** (video
  playing under it, scrolling, dragging). The blurred backdrop is
  recomputed per frame the backdrop changes. With a static desktop
  the same effects measured ~0.7% — identical to baseline.
- `mica`/`micaalt` cost ≈ **-1.5% vs WindowsDefault** — the wallpaper
  is sampled once into a texture; motion under the window adds zero
  cost. Cheapest option, below even the stock backdrop on this build.
- CPU deltas across all effects were noise (~12-21%, unrelated load);
  GPU is the axis that matters for EBM.
- Measurement traps hit: (a) dwm collapses to ~0.6% when the desktop
  goes visually static (e.g. mpv paused) — deltas over baseline are
  the only meaningful reading, not absolutes; (b) `FindWindow` cannot
  see `CabinetWClass` on this system (returns 0 while EnumWindows
  lists the same windows fine — always enumerate); (c) re-enumerate
  the hwnd each move — EBM reloads can recreate the cabinet window
  mid-test.

## Incident — Explorer heap-corruption crash loop on drag (26100.9818)

### Symptoms + evidence

- Moving/dragging a File Explorer window froze then killed explorer;
  the protection banner ("Explorer material paused") fired.
- WER: `Explorer.EXE` × `ntdll.dll` × `0xc0000374` (**heap
  corruption**) at 21:05:45 and 21:06:18 local, on the NEW shell
  build `10.0.26100.9818` (the machine had just updated .9711→.9818).
- `coredesk.log`: `EBML sent` → pid died 6 s later (strike 1),
  re-delivery to the respawned pid → died 18 s later (strike 2) →
  `EBM halted`. Zero explorer crashes after the halt — attribution
  confirmed by cessation.
- Module census of the surviving explorer: only
  `coredesk_tap_v9_*` — no `UWPSpy` residue, no `ExplorerBlurMica`
  (breaker had unloaded it). `ntdll` is the DETECTOR, not the
  writer — EBM's detours are the only third-party writes into
  explorer's heap on this machine.

### Root cause

`ExplorerBlurMica.dll` hardcodes detour targets inside Windows shell
binaries. The UBR bump (9711→9818, same 26100 base) moved them →
hooks patch/read wrong memory → heap corruption that detonates on
window ops (drag = move-event path). NOT a CoreDesk regression —
the vendored engine is build-fragile; any UBR bump can break it.

### Self-heal shipped (file_explorer_styler.rs)

1. **Ban is keyed to `(build+UBR, dll-hash)` — not the effect.** EBM's
   `DllMain` hooks install whatever material the ini asks for, so a
   build that kills `mica` would kill `blur` too. `os_build_tag()`
   = `RtlGetVersion` build + `CurrentVersion\UBR`; `dll` = fnv of the
   deployed bytes. A Windows update OR a shipped engine fix flips a
   key and the contract retries automatically. Records missing the
   new keys (pre-upgrade format) match anything — an old ban keeps
   protecting on first run of the new build. TTL extended to 30d.
2. **DWM-only fallback while banned.** `mica`/`micaalt`/`acrylic`
   need NO injected code — `DwmSetWindowAttribute(hwnd,
   DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_*)` works cross-process on
   `CabinetWClass` (verified live: S_OK + visible mica). Fallback
   maps mica→`MainWindow`, micaalt→`TabbedWindow`, and degrades
   blur/acrylic/blurclear→`TransientWindow` (system acrylic) rather
   than leaving the window bare. Tracked per-hwnd; new cabinets get
   it on the next `ensure()` tick; restored to `DWMSBT_AUTO` on
   re-arm/reset/shutdown (class-checked so recycled hwnds are safe).
3. **`EBM_CRASH_WINDOW` 90 s → 300 s.** Heap corruption detonates on
   USER interaction (drag/snap) which lags delivery — the reported
   crash path sat outside the old window and would never have
   struck. 5 min still bounds attribution.

### Verification

Fresh debug build: old-format record banned EBM on first apply
(`engine stays banned` + `build=26300.9818` — note `RtlGetVersion`
reports 26300 where WER/file versions say 26100; the tag only needs
internal consistency). Next `ensure()` tick applied the DWM mica
fallback to a newly-opened cabinet (`hwnds=1`). `cargo test` green.

### Lessons

- Never trust a third-party injected binary across Windows UBR
  bumps — ban keys must include BOTH the OS revision and the engine
  hash, and the ban must cover the binary, not the feature flag.
- A "no injection needed" fallback turns a crash-looping feature
  into a degraded-but-working one — prefer `DwmSetWindowAttribute`
  on foreign hwnds over `LoadLibrary` whenever the material allows.
- A crash detector's attribution window must cover the real
  detonation trigger (user interaction), not just load time.
- `cap_hwnd`-style captures paint whatever's topmost in the rect —
  `SetWindowPos(TOPMOST)` on a foreign window is not reliable
  z-order proof; verify application via `is_ok()` + log instead.

## Analysis — why the FE window shows no material while EBM is banned

UWPSpy on explorer 14108 confirmed the taskbar WUX island
(`DesktopWindowXamlSource → RootScrollViewer → Taskbar.TaskbarFrame`).
The File Explorer window's live surface map (UIA walk — same data
UWPSpy renders, reachable without injection since CabinetWClass is a
normal top-level hwnd):

    CabinetWClass "Home - File Explorer"
    ├─ TITLE_BAR_SCAFFOLDING_WINDOW_CLASS      custom caption
    ├─ ShellTabWindowClass "Home"              classic DUI surface
    │   └─ DUIViewWndClassName
    │       ├─ ProperTreeHost → SysTreeView32  NAV PANE (classic, opaque)
    │       └─ DUIListView "Shell Folder View"
    │           └─ FileExplorer_FolderViewIsland   WinUI3 file list
    │               └─ Microsoft.UI.Content.DesktopChildSiteBridge
    └─ DesktopChildSiteBridge                  WinUI3 command bar
        └─ TabView + AppBar + AutoSuggestBox

Consequences:

- `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` (the banned-EBM
  fallback) and `SetWindowCompositionAttribute` both affect only the
  window's FRAME/backdrop region — Explorer's client area paints opaque
  over it, so the material is present but invisible. Verified live:
  S_OK + frame-only visual.
- EBM produced the real look because its detours cleared BOTH surface
  kinds: the WinUI3 island backgrounds AND the classic DUI draw paths
  (nav pane / DUIView) — that detour set is precisely what crashes on
  build 26100.9818. There is no attribute-only route to the client.
- The in-process alternative (mux styler clearing the island
  backgrounds) is deliberately UNAVAILABLE in the shared shell
  explorer: `site.rs` drops the winui3 diagnostics session in the
  tray-host pid because advising it marshals SnapLayout's own WinUI3
  island — the documented drag-crash path. Partial transparency (file
  list + command bar only, nav pane stays opaque) would also look
  broken, not just degraded.
- Upstream has no fix to vendor: 2.1.0 is the newest release.

Conclusion: on a build where EBM's detours are incompatible, the
client-area material is unreachable without re-opening a documented
crash path. The breaker + (build,dll) keys keep Explorer stable and
auto-retry the engine when the world changes (Windows update or a
shipped fix) — bounded cost: at most EBM_MAX_STRIKES crashes per new
revision, then silence again.

## Incident #2 — armed WUX diagnostics session is itself the drag-crash trigger

### Reproduction matrix (all kills `0xc0000374` in `ntdll`, stack ends in
`Windows.UI.Xaml` + `combase` marshal — zero `coredesk_tap` frames)

| Configuration | Drag to top edge |
|---|---|
| explorer, zero third-party modules | survived |
| `coredesk_tap` resident, diagnostics ARMED | crashed (5 consecutive) |
| same, after `UNLOAD` (session dead, DLL mapped) | still crashed |
| same, `taskbarStyle=""` (zero writes) | still crashed |
| same, `NoDiag` gate — resident present, no session | survived 4/4 |
| `UWPSpy` attached instead (its own session) | also crashed |

Conclusion: the trigger is ANY armed `IXamlDiagnostics` session inside the
shell pid during drag-time island churn — Microsoft's own marshal
corrupts the heap before/without our callbacks. Not our writes, not the
watch specifics, not EBM.

### Self-heal shipped (NoDiag breaker)

- Resident side (`site.rs`): `diag_disabled()` gates `InitializeXamlDiagnosticsEx`
  on `HKCU\Software\CoreDeskEngine\Debug\NoDiag` + optional
  `NoDiagBuild`/`NoDiagUbr` (HKLM `CurrentBuild`+`UBR` = `RtlGetVersion` build)
  + optional `NoDiagDll` (hash parsed from the resident's own filename —
  `coredesk_tap_v9_<hash>.dll`). Scoped to the shell-tray host: SMEH/NCH/
  Search residents keep diagnostics (their hosts never drag cabinets).
- App side (`taskbar_styler.rs`): `shell_crash_watch` holds a
  `SYNCHRONIZE|QUERY_LIMITED_INFORMATION` handle on the taskbar-owning
  pid; on `Shell_TrayWnd` pid change it judges the dead handle —
  `GetExitCodeProcess >= 0xC0000000` = suspected diagnostics crash
  (WER-mid-dump corpses park in a 60 s pending list). Two strikes →
  `nodiag_write` persists all four keys → `restart_explorer` → the next
  resident installs inert.
- Self-unban: a Windows update (build/UBR mismatch) or a rebuilt resident
  (`NoDiagDll` mismatch) re-arms diagnostics automatically — each new
  image gets one fresh trial, two crashes re-ban it. Verified live:
  `NoDiagDll="deadbeef…"` re-armed (`modern=62` styled), real tag
  re-gated.
- `nodiag_clear_if_stale` sweeps build-keyed flags after an OS update; a
  bare `NoDiag` (manual kill-switch) is never touched.
- `note_shell_respawn` guard refined: when both breakers (`nodiag_active`
  + `ebm_banned`) already inert every explorer-touching surface, the loop
  is "not ours" — the user's styles are kept instead of wiped.

### Verified live (build 26300.9818)

- Armed resident styled `modern=62` elements → two simulated
  exception-exits → `strikes=1`, `strikes=2` → `NoDiag breaker tripped`
  → registry keys written → restart → `IXDEx skipped — NoDiag gate set
  (shell host)` → 4/4 top-edge drags survived.
- Cost on a banned build: taskbar visual styling is OFF (session never
  installs); Start Menu / Notification Center / Search styling
  unaffected. Auto-restores when the keys stop matching.
