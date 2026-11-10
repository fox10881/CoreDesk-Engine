//! Desktop clock widget: a single-instance WebView2 composition host —
//! `WS_POPUP | WS_EX_TOOLWINDOW`, no chrome at all — rendering one
//! self-contained design document per file from `Styles\Clock Widget\`
//! (two-layer resolution: `<exe>` factory first, the user layer in
//! `<config>\Styles` shadows it).
//!
//! Interaction: `WM_NCHITTEST` gives the 8 px perimeter edge/corner
//! resize codes and makes every interior point `HTCAPTION` (full-body
//! drag, supplied by `DefWindowProc`'s `SC_MOVE` loop). `WM_NCLBUTTONDBLCLK`
//! is swallowed so a double-click can't maximize. On `WM_EXITSIZEMOVE`
//! the bounds are pushed as `{"type":"clockMoved"}` over the settings
//! message channel for persistence; the page speaks `{"type":"clockReady"}`
//! on load and takes `{"t":"clockStyle"}` for in-place design swaps.

use crate::commands::{Command, CommandQueue};
use crate::settings_host::WebEnvironment;
use crate::webview_comp::CompWebview;
use crate::widget_doc;
use coredesk_platform::window::{
    self, ClassOptions, MessageHandler, Window, WindowBuilder, WindowClass, style,
};
use coredesk_platform::{HWND, RECT, desktop, frameless, monitors, msg, tray};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_core::Result;

pub const CLOCK_CLASS: &str = "CoreDeskEngine.Clock";
/// Inlined into every widget document's `<script src="i18n.js">` marker —
/// shared by the visualizer's documents too.
pub(crate) const I18N_JS: &str = include_str!("../../../ui/i18n.js");
/// The widget's only context-menu entry (「تحديث」 = soft restart).
const MENU_CLOCK_REFRESH: u32 = 1;

/// First-spawn size in DIPs, centred on the primary work area.
const DEFAULT_W_DIP: i32 = 380;
const DEFAULT_H_DIP: i32 = 220;

struct HostState {
    comp: Option<CompWebview>,
    mouse_tracked: Cell<bool>,
    /// The design file currently navigated — a style swap to a *different*
    /// file re-navigates; same-file re-posts just re-arm the params.
    design_id: RefCell<String>,
}

pub struct ClockHost {
    window: Window,
    state: Rc<RefCell<HostState>>,
}

impl ClockHost {
    pub fn register_class() -> Result<WindowClass> {
        WindowClass::register(CLOCK_CLASS, ClassOptions::default())
    }

    /// Creates the widget hidden (revealed on the page's `clockReady`).
    /// `saved` = persisted `[x, y, w, h]` in physical pixels; `None`
    /// centres `DEFAULT_W_DIP × DEFAULT_H_DIP` on the primary monitor.
    /// `style_id` names the `Styles\Clock Widget\<id>.html` design file.
    pub fn open(
        class: &WindowClass,
        env: &WebEnvironment,
        saved: Option<[i32; 4]>,
        queue: CommandQueue,
        style_id: &str,
    ) -> Result<Self> {
        let state = Rc::new(RefCell::new(HostState {
            comp: None,
            mouse_tracked: Cell::new(false),
            design_id: RefCell::new(String::new()),
        }));

        let (x, y, w, h) = match saved {
            Some([x, y, w, h]) if w > 0 && h > 0 => (x, y, w, h),
            _ => default_bounds(),
        };

        let handler: MessageHandler = {
            let state = state.clone();
            let queue = queue.clone();
            Box::new(
                move |hwnd: HWND, message: u32, wparam: usize, lparam: isize| -> Option<isize> {
                    match message {
                        // Every interior point drags; the 8px perimeter
                        // resizes. DefWindowProc supplies SC_MOVE/SC_SIZE
                        // from the returned codes — nothing is forwarded
                        // to the page, so the widget never eats a drag.
                        msg::WM_NCHITTEST => Some(frameless::widget_hit_test(hwnd, lparam)),
                        // No maximize affordance exists on a chromeless
                        // widget — a double-click must not invent one.
                        msg::WM_NCLBUTTONDBLCLK => Some(0),
                        msg::WM_EXITSIZEMOVE => {
                            let r = window::window_rect(hwnd);
                            queue.push(Command::SettingsMessage(format!(
                                "{{\"type\":\"clockMoved\",\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                                r.left,
                                r.top,
                                r.right - r.left,
                                r.bottom - r.top
                            )));
                            Some(0)
                        }
                        msg::WM_SIZE => {
                            let w = msg::lo_i16(lparam);
                            let h = msg::hi_i16(lparam);
                            if let Some(c) = state.borrow().comp.as_ref() {
                                c.set_bounds(w, h);
                                // The WebView2 input twin re-anchors on both
                                // move AND size — without this a resize-only
                                // SetWindowPos strands it at the old rect as a
                                // black frame.
                                c.notify_parent_window_position_changed();
                            }
                            Some(0)
                        }
                        msg::WM_MOVE => {
                            if let Some(c) = state.borrow().comp.as_ref() {
                                c.notify_parent_window_position_changed();
                            }
                            Some(0)
                        }
                        // Never paint the surface: the composition visual
                        // composites transparency over it.
                        msg::WM_ERASEBKGND => Some(1),
                        msg::WM_DPICHANGED => {
                            // SAFETY: lParam points to the suggested RECT for
                            // the duration of the message.
                            let r = unsafe { *(lparam as *const RECT) };
                            let _ = window::set_window_bounds(
                                hwnd,
                                r.left,
                                r.top,
                                r.right - r.left,
                                r.bottom - r.top,
                            );
                            if let Some(c) = state.borrow().comp.as_ref() {
                                c.set_rasterization_scale(window::dpi_of(hwnd));
                            }
                            Some(0)
                        }
                        msg::WM_SETCURSOR => {
                            if (lparam & 0xffff) as i64 != msg::HTCLIENT as i64 {
                                return None;
                            }
                            let s = state.borrow();
                            if let Some(c) = s.comp.as_ref() {
                                let cursor = c.cursor();
                                window::set_cursor(if cursor.is_null() {
                                    let id = c.system_cursor_id();
                                    window::load_system_cursor(if id == 0 { 32512 } else { id })
                                } else {
                                    cursor
                                });
                            }
                            Some(1)
                        }
                        // The widget's only chrome: right-click shows a
                        // single-item 「تحديث」 menu — the manual fail-safe
                        // that soft-restarts a broken (opaque-black) WebView
                        // surface. Interior hit-tests as HTCAPTION, so clicks
                        // arrive as WM_NCRBUTTONUP; the keyboard menu key
                        // arrives as WM_CONTEXTMENU (x=y=-1 → centre).
                        msg::WM_CONTEXTMENU | msg::WM_NCRBUTTONUP => {
                            let x = msg::lo_i16(lparam);
                            let y = msg::hi_i16(lparam);
                            let (x, y) = if (x, y) == (-1, -1) {
                                let r = window::window_rect(hwnd);
                                ((r.left + r.right) / 2, (r.top + r.bottom) / 2)
                            } else {
                                (x, y)
                            };
                            let menu = tray::PopupMenu::new();
                            menu.item(MENU_CLOCK_REFRESH, "تحديث", false, false);
                            if menu.show_context(hwnd, x, y) == MENU_CLOCK_REFRESH {
                                queue.push(Command::RefreshClock);
                            }
                            Some(0)
                        }
                        msg::WM_DESTROY => {
                            let mut s = state.borrow_mut();
                            if let Some(c) = s.comp.take() {
                                c.close();
                            }
                            queue.push(Command::WindowGone(hwnd));
                            Some(0)
                        }
                        m if m == msg::WM_MOUSEMOVE
                            || (0x0201..=0x020E).contains(&m)
                            || m == msg::WM_MOUSELEAVE =>
                        {
                            // Interior points hit-test as HTCAPTION so these
                            // arrive only for the (rare) true client case —
                            // never a WM_NC* message, which the controller
                            // cannot consume.
                            let s = state.borrow();
                            match s.comp.as_ref() {
                                Some(c) => {
                                    c.forward_wm(hwnd, m, wparam, lparam, &s.mouse_tracked);
                                    Some(0)
                                }
                                None => None,
                            }
                        }
                        _ => None,
                    }
                },
            )
        };

        let window = WindowBuilder::new(class)
            .style(style::POPUP | style::CLIPCHILDREN)
            .ex_style(style::EX_TOOLWINDOW | style::EX_NOACTIVATE)
            .bounds(x, y, w, h)
            .create(handler)?;
        let hwnd = window.hwnd();
        tracing::debug!(?hwnd, x, y, w, h, "clock widget host created");

        let Some(document) =
            widget_doc::build_widget_document(coredesk_core::styles_fs::CLOCK_WIDGET_DIR, style_id)
        else {
            tracing::warn!(style_id, "clock design file missing — widget not spawned");
            return Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                -2147024894, // HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)
            )));
        };

        let (cw, ch) = window.client_size();
        let comp = {
            let queue = queue.clone();
            CompWebview::create(
                env.inner(),
                hwnd,
                window::dpi_of(hwnd),
                (cw, ch),
                &document,
                move |json| {
                    if widget_doc::widget_message_allowed(&json, &["clockReady"]) {
                        queue.push(Command::SettingsMessage(json));
                    }
                },
            )?
        };
        *state.borrow_mut().design_id.borrow_mut() = style_id.to_string();
        state.borrow_mut().comp = Some(comp);
        Ok(Self { window, state })
    }

    /// Live design swap: navigates the running WebView to the design's
    /// file. The fresh page's `clockReady` re-arms style/params/theme —
    /// `false` when the file can't be resolved (caller keeps or closes the
    /// widget per its own policy).
    pub fn set_design(&self, style_id: &str) -> bool {
        if *self.state.borrow().design_id.borrow() == style_id {
            return true;
        }
        let Some(document) =
            widget_doc::build_widget_document(coredesk_core::styles_fs::CLOCK_WIDGET_DIR, style_id)
        else {
            tracing::warn!(style_id, "clock design file missing — swap ignored");
            return false;
        };
        let ok = self
            .state
            .borrow()
            .comp
            .as_ref()
            .is_some_and(|c| c.navigate_to_string(&document).is_ok());
        if ok {
            *self.state.borrow().design_id.borrow_mut() = style_id.to_string();
        }
        ok
    }

    pub fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    pub fn post_json(&self, json: &str) {
        if let Some(c) = self.state.borrow().comp.as_ref() {
            c.post_json(json);
        }
    }

    /// Re-asserts the WebView's transparent surface + DComp attach after a
    /// desktop composition-band rebuild — the in-place counterpart of the
    /// soft restart (`CompWebview::rearm_transparency`).
    pub fn rearm_transparency(&self) {
        if let Some(c) = self.state.borrow().comp.as_ref() {
            c.rearm_transparency(
                window::dpi_of(self.window.hwnd()),
                self.window.client_size(),
            );
        }
    }

    /// Re-pins a lost companion without a respawn: uncloaks the host,
    /// then re-attaches the DComp visual and re-notifies the position —
    /// the notify resurrects a parked companion where a plain show does
    /// not.
    pub fn revive_surface(&self) {
        let hwnd = self.window.hwnd();
        if desktop::is_cloaked(hwnd) {
            let _ = desktop::set_cloak(hwnd, false);
        }
        self.rearm_transparency();
    }

    /// Language switched while the widget is up: push a fresh
    /// `{t:"clockLocale", locale, translations}` so the page rebuilds its
    /// Intl formatters without recreating the window.
    pub fn update_language(&self) {
        let mut p = coredesk_core::i18n::ui_payload();
        if let Some(o) = p.as_object_mut() {
            o.insert("t".to_string(), "clockLocale".into());
        }
        self.post_json(&p.to_string());
    }

    /// Closes the browser and destroys the window.
    pub fn close(self) {
        {
            let mut s = self.state.borrow_mut();
            if let Some(c) = s.comp.take() {
                c.close();
            }
        }
        drop(self.window);
    }
}

const RECT_FALLBACK: coredesk_platform::RECT = coredesk_platform::RECT {
    left: 0,
    top: 0,
    right: 1920,
    bottom: 1040,
};

/// First-spawn bounds: `DEFAULT_W_DIP × DEFAULT_H_DIP` scaled to the primary
/// monitor's DPI and centred on its work area. Also used when a theme's
/// `clockBounds` is absent so the widget returns to its default spot.
pub fn default_bounds() -> (i32, i32, i32, i32) {
    let mons = monitors::enumerate();
    let primary = mons.iter().find(|m| m.primary).or(mons.first());
    let scale = primary.map(|m| m.scale()).unwrap_or(1.0);
    let area = primary.map(|m| m.work_area).unwrap_or(RECT_FALLBACK);
    let (w, h) = (
        (DEFAULT_W_DIP as f32 * scale) as i32,
        (DEFAULT_H_DIP as f32 * scale) as i32,
    );
    (
        area.left + (area.right - area.left - w) / 2,
        area.top + (area.bottom - area.top - h) / 2,
        w,
        h,
    )
}
