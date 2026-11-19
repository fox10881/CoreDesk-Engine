//! Taskbar resource widget: a native Direct2D/DirectWrite overlay strip
//! sitting on the taskbar's free left area — zero WebView, zero browser
//! cost. One `WS_POPUP | WS_EX_NOACTIVATE` window composited through a
//! [`Panel`] (premultiplied-alpha D2D surface) parked `HWND_TOPMOST` over
//! `Shell_TrayWnd`'s band, fed by [`PerfProbe`]'s shared snapshot at 1 Hz.
//!
//! The strip owns three timers on its own HWND:
//! - `TIMER_TICK` (1 s): read the snapshot → rebuild the text → `Panel::draw`.
//! - `TIMER_TRACK` (500 ms): taskbar rect moved / z-order slipped /
//!   auto-hide retracted / a fullscreen window covered the band →
//!   reposition, re-assert topmost, or hide.
//! - The backdrop probe runs off-thread (`backdrop_probe::sample` blocks)
//!   and lands as a boxed `Sample` on `WM_TBW_SAMPLE` — event-driven only
//!   (arm, drag end, style swap), matching the clock widget's cadence.
//!
//! Drag: `WM_NCHITTEST` returns `HTCAPTION` across the whole strip so
//! `DefWindowProc` runs a native move loop; `WM_MOVING` clamps the rect
//! onto the taskbar's left quarter so it can never cover app icons.
//! `locked` flips interior hits to `HTCLIENT` — the context menu (قفل
//! الموقع / تحديث) stays reachable while the widget stops moving.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use coredesk_core::taskbar_widget_styles::{Metric, TbWidgetStyle};
use coredesk_platform::window::{
    self, ClassOptions, MessageHandler, Window, WindowBuilder, WindowClass, style,
};
use coredesk_platform::{
    HWND, RECT, backdrop_probe, desktop, monitors, msg, perf_probe::PerfSnapshot, taskbar_area,
    tray,
};
use coredesk_render::{
    Brush, ColorF, FontWeight, Panel, Rect, RenderStack, TextFormat, TextLayout,
};
use windows_composition::{ContainerVisual, DesktopWindowTarget};
use windows_core::Result;

use crate::commands::{Command, CommandQueue};

pub const TASKBAR_WIDGET_CLASS: &str = "CoreDeskEngine.TaskbarWidget";

const TIMER_TICK: usize = 1;
const TIMER_TRACK: usize = 2;
const TICK_MS: u32 = 1_000;
const TRACK_MS: u32 = 500;

/// Backdrop-probe delivery (`WM_APP` range): lParam = `Box<Sample>` raw.
const WM_TBW_SAMPLE: u32 = 0x8000 + 0x51;

const MENU_REFRESH: u32 = 1;
const MENU_LOCK: u32 = 2;

/// Drag clamp: the widget may only live inside the taskbar's left quarter.
const LEFT_BAND_DIV: i32 = 4;
/// Default left-edge margin in DIPs (scaled by the taskbar's monitor DPI).
const DEFAULT_MARGIN_DIP: f32 = 10.0;

struct Core {
    window: Option<Window>,
    _target: Option<DesktopWindowTarget>,
    root: Option<ContainerVisual>,
    panel: Option<Panel>,
    /// Shared read side of the PDH sampler (the app owns the probe).
    snap: Arc<RwLock<PerfSnapshot>>,
    style: TbWidgetStyle,
    /// Resolved draw colors — authored, or backdrop-derived when "auto".
    ink: ColorF,
    accent: ColorF,
    shadow_ink: ColorF,
    /// Cached label/value formats (rebuilt on style or DPI change).
    fmt: Option<(TextFormat, TextFormat)>,
    /// Left-edge offset in physical px from `Shell_TrayWnd`'s left.
    offset: i32,
    locked: bool,
    occluded: bool,
    queue: CommandQueue,
    dpi: u32,
    device_lost: bool,
}

/// The app-facing handle — `Rc` state lives on the window's thread.
pub struct TaskbarWidgetHost {
    class: WindowClass,
    stack: Rc<RenderStack>,
    core: Rc<RefCell<Core>>,
}

impl TaskbarWidgetHost {
    pub fn register_class() -> Result<WindowClass> {
        WindowClass::register(TASKBAR_WIDGET_CLASS, ClassOptions::default())
    }

    pub fn new(class: WindowClass, stack: Rc<RenderStack>, queue: CommandQueue) -> Self {
        Self {
            class,
            stack,
            core: Rc::new(RefCell::new(Core {
                window: None,
                _target: None,
                root: None,
                panel: None,
                snap: Arc::new(RwLock::new(PerfSnapshot::default())),
                style: TbWidgetStyle::default(),
                ink: ColorF::new(1.0, 1.0, 1.0, 1.0),
                accent: ColorF::new(0.62, 0.75, 0.97, 1.0),
                shadow_ink: ColorF::new(0.0, 0.0, 0.0, 0.55),
                fmt: None,
                offset: 0,
                locked: false,
                occluded: false,
                queue,
                dpi: 96,
                device_lost: false,
            })),
        }
    }

    pub fn hwnd(&self) -> Option<HWND> {
        self.core.borrow().window.as_ref().map(|w| w.hwnd())
    }

    /// Arm the widget: `style` is the parsed `.json` descriptor, `offset`
    /// the persisted left-edge px (`0`/`negatives` fall back to the margin),
    /// `snap` the app's shared perf feed.
    pub fn open(
        &self,
        style: TbWidgetStyle,
        offset: i32,
        locked: bool,
        snap: Arc<RwLock<PerfSnapshot>>,
    ) -> Result<()> {
        self.close();
        {
            let mut c = self.core.borrow_mut();
            c.style = style;
            c.offset = offset;
            c.locked = locked;
            c.snap = snap;
            c.occluded = false;
            c.fmt = None;
        }

        let Some(bar) = taskbar_area::visible_rect() else {
            tracing::warn!("tb-widget: no taskbar rect — widget not spawned");
            return Ok(());
        };

        let core = self.core.clone();
        let handler: MessageHandler = Box::new(
            move |hwnd: HWND, message: u32, wparam: usize, lparam: isize| -> Option<isize> {
                // Hit-testing never waits on the engine borrow — the answer
                // must be instant or the taskbar under us eats the click.
                if message == msg::WM_NCHITTEST {
                    let locked = core.try_borrow().is_ok_and(|c| c.locked);
                    // Locked: client hits keep the context menu reachable
                    // without the drag affordance; unlocked: the whole strip
                    // is a caption (DefWindowProc supplies the SC_MOVE loop).
                    return Some(if locked {
                        msg::HTCLIENT
                    } else {
                        msg::HTCAPTION
                    });
                }
                let mut c = match core.try_borrow_mut() {
                    Ok(c) => c,
                    Err(_) => return None,
                };
                match message {
                    msg::WM_TIMER => {
                        match wparam {
                            TIMER_TICK => c.tick(),
                            TIMER_TRACK => c.track(),
                            _ => {}
                        }
                        Some(0)
                    }
                    msg::WM_MOVING => {
                        // SAFETY: lParam is the in-out rect for the move loop.
                        let r = unsafe { &mut *(lparam as *mut RECT) };
                        c.clamp_moving(r);
                        Some(0)
                    }
                    msg::WM_EXITSIZEMOVE => {
                        c.persist_offset();
                        c.probe_backdrop();
                        Some(0)
                    }
                    msg::WM_DPICHANGED => {
                        c.dpi = window::dpi_of(hwnd);
                        c.fmt = None;
                        // SAFETY: lParam is the suggested rect.
                        let r = unsafe { *(lparam as *const RECT) };
                        let _ = window::set_window_bounds(
                            hwnd,
                            r.left,
                            r.top,
                            r.right - r.left,
                            r.bottom - r.top,
                        );
                        Some(0)
                    }
                    msg::WM_ERASEBKGND => Some(1),
                    msg::WM_CONTEXTMENU | msg::WM_NCRBUTTONUP => {
                        let (mut x, mut y) = (msg::lo_i16(lparam), msg::hi_i16(lparam));
                        if (x, y) == (-1, -1) {
                            let r = window::window_rect(hwnd);
                            (x, y) = ((r.left + r.right) / 2, r.top - 8);
                        }
                        let menu = tray::PopupMenu::new();
                        menu.item(
                            MENU_REFRESH,
                            coredesk_core::i18n::text("刷新"),
                            false,
                            false,
                        );
                        menu.item(
                            MENU_LOCK,
                            coredesk_core::i18n::text("锁定位置"),
                            c.locked,
                            false,
                        );
                        let pick = menu.show_context(hwnd, x, y);
                        drop(c);
                        match pick {
                            MENU_REFRESH => core.borrow().queue.push(Command::RefreshTaskbarWidget),
                            MENU_LOCK => core.borrow().queue.push(Command::ToggleTaskbarWidgetLock),
                            _ => {}
                        }
                        Some(0)
                    }
                    m if m == WM_TBW_SAMPLE => {
                        // lparam is a publish() token — a forged id finds
                        // no parked sample and the message is dropped.
                        if let Some(sample) = backdrop_probe::take(lparam) {
                            c.apply_sample(sample);
                        }
                        Some(0)
                    }
                    msg::WM_DESTROY => {
                        c.window = None;
                        c.queue.push(Command::WindowGone(hwnd));
                        Some(0)
                    }
                    _ => None,
                }
            },
        );

        let rect = {
            let mut c = self.core.borrow_mut();
            desired_rect(&mut c, bar)
        };
        let window = WindowBuilder::new(&self.class)
            .title("CoreDesk Engine Taskbar Widget")
            .style(style::POPUP | style::CLIPCHILDREN)
            .ex_style(style::EX_NOACTIVATE | style::EX_TOOLWINDOW | style::EX_NOREDIRECTIONBITMAP)
            .bounds(rect.0, rect.1, rect.2, rect.3)
            .create(handler)?;
        let hwnd = window.hwnd();
        let _ = coredesk_platform::dwm::set_transitions_disabled(hwnd, true);

        // Composition tree: target → container → Panel sprite (identical to
        // the effects overlay — alpha comes from the visual, never layered).
        let target = self.stack.create_target(hwnd)?;
        let root = self.stack.compositor.create_container_visual();
        let panel = Panel::new(&self.stack)?;
        root.children().insert_at_top(&panel.visual);
        target.set_root(&root);

        {
            let mut c = self.core.borrow_mut();
            c.dpi = window::dpi_of(hwnd);
            c._target = Some(target);
            c.root = Some(root);
            c.panel = Some(panel);
            c.window = Some(window);
        }
        // Resize the surface to the strip, then show+arm timers.
        self.resize_surface();
        let c = self.core.borrow_mut();
        let w = c.window.as_ref().unwrap();
        w.show_no_activate();
        let _ = desktop::set_topmost(hwnd, true);
        w.set_timer(TIMER_TICK, TICK_MS);
        w.set_timer(TIMER_TRACK, TRACK_MS);
        drop(c);
        // First paint + first backdrop probe now (event-driven thereafter).
        {
            let mut c = self.core.borrow_mut();
            c.tick();
            c.probe_backdrop();
        }
        tracing::info!(?rect, "tb-widget: overlay opened");
        Ok(())
    }

    /// Re-arm style/lock/offset on a live widget (Settings apply or theme
    /// swap) — no respawn needed: formats rebuild lazily on next tick.
    pub fn apply(&self, style: TbWidgetStyle, offset: i32, locked: bool) {
        let mut c = self.core.borrow_mut();
        c.style = style;
        c.offset = offset;
        c.locked = locked;
        c.fmt = None;
        c.reposition();
        c.tick();
        c.probe_backdrop();
    }

    /// Toggle the drag lock on the live widget.
    pub fn set_locked(&self, locked: bool) {
        self.core.borrow_mut().locked = locked;
    }

    /// The panel's GPU surface died with the render stack — rebuild it on
    /// the fresh device (same recovery path as `EffectsHost`).
    pub fn recreate_surfaces(&self) {
        let mut c = self.core.borrow_mut();
        if let Some(p) = c.panel.as_mut()
            && p.recreate(&self.stack).is_ok()
        {
            c.device_lost = false;
        }
    }

    pub fn device_lost(&self) -> bool {
        self.core.borrow().device_lost
    }

    /// Resizes the surface to the strip's measured text — the window's
    /// width tracks content (metric set + DPI + font).
    fn resize_surface(&self) {
        let mut c = self.core.borrow_mut();
        let Some(hwnd) = c.window.as_ref().map(|w| w.hwnd()) else {
            return;
        };
        let Some(bar) = taskbar_area::visible_rect() else {
            return;
        };
        let rect = desired_rect(&mut c, bar);
        let _ = window::set_window_bounds(hwnd, rect.0, rect.1, rect.2, rect.3);
        if let Some(p) = c.panel.as_mut() {
            let _ = p.resize(rect.2, rect.3);
        }
    }

    /// Destroy the strip — the "none" state releases every GPU resource.
    pub fn close(&self) {
        let mut c = self.core.borrow_mut();
        if let Some(w) = c.window.as_ref() {
            w.kill_timer(TIMER_TICK);
            w.kill_timer(TIMER_TRACK);
        }
        if let Some(root) = c.root.take()
            && let Some(p) = c.panel.take()
        {
            root.children().remove(&p.visual);
        }
        c._target = None;
        c.occluded = false;
        c.window = None; // drops last → DestroyWindow after detach
    }
}

/// The strip's rect: `visible_rect` band, `offset` px from its left edge,
/// clamped inside the left quarter so it can never cover taskbar icons.
fn desired_rect(c: &mut Core, bar: RECT) -> (i32, i32, i32, i32) {
    let scale = c.dpi.max(96) as f32 / 96.0;
    let text_dip = c.worst_text_width();
    let w = ((c.style.padding_x * 2.0 + text_dip) * scale).ceil() as i32 + 2;
    let h = bar.bottom - bar.top;
    let band_w = (bar.right - bar.left) / LEFT_BAND_DIV;
    // Negative offset = "never dragged": park at the style's left margin.
    let left = if c.offset >= 0 {
        bar.left + c.offset
    } else {
        bar.left + (DEFAULT_MARGIN_DIP * scale).round() as i32
    };
    let x = left.min(bar.left + (band_w - w).max(0));
    (x, bar.top, w.max(24), h)
}

impl Core {
    /// Widest the text can ever get with this metric set — sizes the strip
    /// once so the window never churns on live digits ("100%" worst case).
    fn worst_text_width(&mut self) -> f32 {
        let (lfmt, vfmt) = self.formats();
        let mut w = 0.0f32;
        for (i, m) in self.style.metrics.iter().enumerate() {
            if i > 0 {
                w += measure(&self.style.separator, &lfmt);
            }
            w += measure(metric_label(*m), &lfmt);
            w += measure(metric_worst(*m), &vfmt);
        }
        w
    }

    /// Cached (label, value) formats — value runs semibold accent.
    fn formats(&mut self) -> (TextFormat, TextFormat) {
        if self.fmt.is_none() {
            let label = TextFormat::with_weight(
                &self.style.font,
                self.style.font_size,
                FontWeight(self.style.font_weight as i32),
            )
            .unwrap_or_else(|_| TextFormat::new(&self.style.font, self.style.font_size).unwrap());
            let value =
                TextFormat::with_weight(&self.style.font, self.style.font_size, FontWeight(700))
                    .unwrap_or_else(|_| {
                        TextFormat::new(&self.style.font, self.style.font_size).unwrap()
                    });
            self.fmt = Some((label, value));
        }
        self.fmt.clone().unwrap()
    }

    /// The live text for one metric: `("CPU", " 12%")`-shaped label/value
    /// pairs so the value can take the accent color.
    fn segments(&self) -> Vec<(String, String)> {
        let s = *self.snap.read().unwrap_or_else(|p| p.into_inner());
        self.style
            .metrics
            .iter()
            .map(|m| (metric_label(*m).to_string(), metric_value(*m, &s)))
            .collect()
    }

    /// 1 Hz: redraw the strip from the shared snapshot.
    fn tick(&mut self) {
        // Clone everything the closure needs *before* borrowing the panel —
        // `formats()`/`segments()` take `&mut`/`&` self while `draw` runs.
        let (lfmt, vfmt) = self.formats();
        let segs = self.segments();
        let sep = self.style.separator.clone();
        let pad = self.style.padding_x;
        let shadow = self.style.shadow;
        let (ink, accent, shadow_ink) = (self.ink, self.accent, self.shadow_ink);
        let dpi = self.dpi;
        let Some(panel) = self.panel.as_ref() else {
            return;
        };
        let ok = panel.draw(dpi, |s, _w, h| {
            // The swap chain keeps prior pixels — wipe to transparent or
            // every tick's text piles onto the last frame (ghost glyphs).
            s.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
            let ink_b = s.create_solid_brush(ink)?;
            let acc_b = s.create_solid_brush(accent)?;
            // DrawText is top-aligned in its box: centre the line inside
            // the strip's full height instead of pinning it to the top.
            let line_h = TextLayout::new("Ag", &lfmt, 10_000.0, 10_000.0)
                .map(|l| l.metrics().height)
                .unwrap_or(h);
            let y = ((h - line_h) / 2.0).max(0.0);
            let draw_at = |dx: f32, dy: f32, brush: &Brush| {
                let mut x = pad + dx;
                for (i, (label, value)) in segs.iter().enumerate() {
                    if i > 0 {
                        s.draw_text(&sep, &lfmt, &dip_rect(x, y + dy, h), brush);
                        x += measure(&sep, &lfmt);
                    }
                    s.draw_text(label, &lfmt, &dip_rect(x, y + dy, h), brush);
                    x += measure(label, &lfmt);
                    s.draw_text(value, &vfmt, &dip_rect(x, y + dy, h), brush);
                    x += measure(value, &vfmt);
                }
            };
            if shadow {
                let sh = s.create_solid_brush(shadow_ink)?;
                draw_at(0.0, 1.0, &sh);
            }
            draw_at(0.0, 0.0, &ink_b);
            // Second pass would need per-part brushes — the accent pass
            // redraws values only.
            let mut x = pad;
            for (i, (label, value)) in segs.iter().enumerate() {
                if i > 0 {
                    x += measure(&sep, &lfmt);
                }
                x += measure(label, &lfmt);
                s.draw_text(value, &vfmt, &dip_rect(x, y, h), &acc_b);
                x += measure(value, &vfmt);
            }
            Ok(())
        });
        match ok {
            Ok(true) => {}
            Ok(false) => {
                if !self.device_lost {
                    self.device_lost = true;
                    self.queue.push(Command::DeviceLost);
                }
            }
            Err(e) => tracing::warn!(error = %e, "tb-widget: draw failed"),
        }
    }

    /// 500 ms: keep the strip glued to the taskbar — rect moved (theme
    /// apply, autohide, monitor change), z-order slipped behind the bar,
    /// or a fullscreen window covered the band.
    fn track(&mut self) {
        let Some(hwnd) = self.window.as_ref().map(|w| w.hwnd()) else {
            return;
        };
        let Some(bar) = taskbar_area::visible_rect() else {
            self.set_occluded(true);
            return;
        };
        if taskbar_area::is_vertical() || taskbar_area::covered(bar, hwnd) {
            self.set_occluded(true);
            return;
        }
        self.set_occluded(false);
        // Re-assert topmost: the taskbar's own band can raise above us on
        // shell churn — idempotent, no flicker.
        let _ = desktop::set_topmost(hwnd, true);
        let rect = desired_rect(self, bar);
        let cur = window::window_rect(hwnd);
        if cur.left != rect.0
            || cur.top != rect.1
            || cur.right - cur.left != rect.2
            || cur.bottom - cur.top != rect.3
        {
            let _ = window::set_window_bounds(hwnd, rect.0, rect.1, rect.2, rect.3);
            if let Some(p) = self.panel.as_mut() {
                let _ = p.resize(rect.2, rect.3);
            }
        }
    }

    /// Hide/show the strip with the taskbar's visibility — hidden costs
    /// nothing (the 1 Hz tick still draws into a hidden surface, but the
    /// compositor never commits it).
    fn set_occluded(&mut self, occluded: bool) {
        if self.occluded == occluded {
            return;
        }
        self.occluded = occluded;
        if let Some(w) = self.window.as_ref() {
            if occluded {
                w.hide();
            } else {
                w.show_no_activate();
            }
        }
    }

    /// `WM_MOVING`: clamp the drag rect onto the taskbar's left band —
    /// vertical is pinned to the band's top; horizontal can't leave the
    /// left quarter (icons live to the right of it).
    fn clamp_moving(&mut self, r: &mut RECT) {
        let Some(bar) = taskbar_area::visible_rect() else {
            return;
        };
        let w = r.right - r.left;
        let band_w = (bar.right - bar.left) / LEFT_BAND_DIV;
        let max_x = bar.left + (band_w - w).max(0);
        let x = r.left.clamp(bar.left, max_x);
        r.left = x;
        r.right = x + w;
        r.top = bar.top;
        r.bottom = bar.top + (bar.bottom - bar.top);
    }

    /// Drag ended: persist `x - taskbar.left` for the settings layer.
    fn persist_offset(&mut self) {
        let Some(w) = self.window.as_ref() else {
            return;
        };
        let Some(bar) = taskbar_area::primary_rect() else {
            return;
        };
        let cur = window::window_rect(w.hwnd());
        self.offset = cur.left - bar.left;
        self.queue.push(Command::SettingsMessage(format!(
            "{{\"type\":\"tbWidgetMoved\",\"offset\":{}}}",
            self.offset
        )));
    }

    /// Snap the strip back onto its band (settings/theme apply paths).
    fn reposition(&mut self) {
        let Some(hwnd) = self.window.as_ref().map(|w| w.hwnd()) else {
            return;
        };
        let Some(bar) = taskbar_area::visible_rect() else {
            return;
        };
        let rect = desired_rect(self, bar);
        let _ = window::set_window_bounds(hwnd, rect.0, rect.1, rect.2, rect.3);
        if let Some(p) = self.panel.as_mut() {
            let _ = p.resize(rect.2, rect.3);
        }
    }

    /// Event-driven backdrop sample: what shows *behind* the strip (the
    /// taskbar's own tinted/transparent surface) decides ink+accent so the
    /// widget reads as part of the bar on every theme — off-thread, the
    /// boxed `Sample` returns on `WM_TBW_SAMPLE`.
    fn probe_backdrop(&mut self) {
        let Some(w) = self.window.as_ref() else {
            return;
        };
        if self.style.text_color.is_some() && self.style.accent_color.is_some() {
            // Fully authored colors — no sampling needed.
            self.apply_sample(backdrop_probe::Sample {
                luminance: 0,
                accent: [0x9E, 0xC0, 0xFA],
            });
            return;
        }
        let (rect, hwnd) = (window::window_rect(w.hwnd()), w.hwnd().0 as isize);
        std::thread::Builder::new()
            .name("coredesk-tbw-probe".into())
            .spawn(move || {
                let _com = coredesk_platform::com::MtaGuard::init();
                let sample = backdrop_probe::sample(HWND(hwnd as _), rect, &monitors::enumerate());
                let token = backdrop_probe::publish(sample);
                window::post_message(HWND(hwnd as _), WM_TBW_SAMPLE, 0, token);
            })
            .ok();
    }

    /// Resolve the sample (or authored colors) into the draw palette:
    /// luminance picks the light-on-dark/dark-on-light ink exactly like
    /// Windows' own taskbar text; accent follows the backdrop's dominant
    /// hue unless the style pinned both.
    fn apply_sample(&mut self, sample: backdrop_probe::Sample) {
        let dark_backdrop = sample.luminance < 140;
        self.ink = self
            .style
            .text_color
            .map(|c| cf(c, 1.0))
            .unwrap_or(if dark_backdrop {
                ColorF::new(0.96, 0.97, 0.98, 1.0)
            } else {
                ColorF::new(0.08, 0.09, 0.11, 1.0)
            });
        self.accent = self
            .style
            .accent_color
            .map(|c| cf(c, 1.0))
            .unwrap_or_else(|| cf(sample.accent, 1.0));
        self.shadow_ink = if dark_backdrop {
            ColorF::new(0.0, 0.0, 0.0, 0.55)
        } else {
            ColorF::new(1.0, 1.0, 1.0, 0.6)
        };
        self.tick();
    }
}

fn cf(rgb: [u8; 3], a: f32) -> ColorF {
    ColorF {
        r: rgb[0] as f32 / 255.0,
        g: rgb[1] as f32 / 255.0,
        b: rgb[2] as f32 / 255.0,
        a,
    }
}

fn metric_label(m: Metric) -> &'static str {
    match m {
        Metric::Cpu => "CPU",
        Metric::Ram => "RAM",
        Metric::Gpu => "GPU",
        Metric::Net => "NET",
    }
}

/// Worst-case value text — the strip is sized once on this so live digits
/// never resize the window.
fn metric_worst(m: Metric) -> &'static str {
    match m {
        Metric::Cpu | Metric::Ram | Metric::Gpu => " 100%",
        Metric::Net => " 999M↓ 999M↑",
    }
}

fn metric_value(m: Metric, s: &PerfSnapshot) -> String {
    match m {
        Metric::Cpu => format!(" {:.0}%", s.cpu.min(999.0)),
        Metric::Ram => format!(" {:.0}%", s.ram.min(999.0)),
        Metric::Gpu => format!(" {:.0}%", s.gpu.min(999.0)),
        Metric::Net => format!(" {}↓ {}↑", fmt_rate(s.down_bps), fmt_rate(s.up_bps)),
    }
}

fn fmt_rate(bps: f64) -> String {
    if bps >= 1_000_000.0 {
        format!("{:.0}M", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0}K", bps / 1_000.0)
    } else {
        format!("{:.0}", bps)
    }
}

fn measure(text: &str, format: &TextFormat) -> f32 {
    TextLayout::new(text, format, 10_000.0, 10_000.0)
        .map(|l| l.metrics().width_including_trailing_whitespace)
        .unwrap_or(0.0)
}

fn dip_rect(x: f32, y: f32, h: f32) -> Rect {
    Rect {
        left: x,
        top: y,
        right: x + 10_000.0,
        bottom: y + h,
    }
}
