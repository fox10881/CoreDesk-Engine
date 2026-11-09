//! Desktop audio-visualizer widget: a single WebView2 composition host —
//! `WS_POPUP | WS_EX_TOOLWINDOW`, no chrome — rendering one self-contained
//! design document per file from `Styles\Visualizer Widget\` (two-layer
//! resolution: `<exe>` factory first, the user layer in `<config>\Styles`
//! shadows it).
//!
//! Same interaction model as the clock widget (`WM_NCHITTEST` full-body drag +
//! 8 px edge resize, `vizMoved` persists bounds on `WM_EXITSIZEMOVE`). The
//! difference is the feed: a `WM_TIMER` on the host window pushes
//! [`SpectrumSnapshot::json`] at the engine's `TICK_MS` cadence straight from
//! the engine's shared spectrum — while `paused` is set (occluded desktop) only
//! the edge frame posts, so an invisible visualizer does zero WebView work.

use crate::commands::{Command, CommandQueue};
use crate::settings_host::WebEnvironment;
use crate::visualizer_engine::SpectrumSnapshot;
use crate::webview_comp::CompWebview;
use crate::widget_doc;
use coredesk_platform::window::{
    self, ClassOptions, MessageHandler, Window, WindowBuilder, WindowClass, style,
};
use coredesk_platform::{HWND, RECT, frameless, monitors, msg, tray};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use windows_core::Result;

pub const VISUALIZER_CLASS: &str = "CoreDeskEngine.Visualizer";
/// The widget's only context-menu entry (「تحديث」 = soft restart).
const MENU_VIZ_REFRESH: u32 = 1;
/// Host-window timer: pushes a spectrum frame every engine tick (~33 ms).
const TIMER_FEED: usize = 1;
const FEED_MS: u32 = 33;

/// First-spawn size in DIPs, centred on the primary work area — a wide strip
/// suits every design; per-design rects refine via `widget_params` bounds.
const DEFAULT_W_DIP: i32 = 560;
const DEFAULT_H_DIP: i32 = 200;

struct HostState {
    comp: Option<CompWebview>,
    mouse_tracked: Cell<bool>,
    /// Shared read side of the engine's spectrum (engine owns the writer).
    spectrum: Arc<RwLock<SpectrumSnapshot>>,
    /// One `paused:true` frame is posted on the occlusion edge so the page
    /// freezes deliberately — then posting stops until the desktop is visible
    /// again. Nothing rendering-related runs while this is `true`.
    posted_paused: Cell<bool>,
    /// The design file currently navigated — a style swap to a *different*
    /// file re-navigates; same-file re-posts just re-arm the params.
    design_id: RefCell<String>,
}

pub struct VisualizerHost {
    window: Window,
    state: Rc<RefCell<HostState>>,
}

impl VisualizerHost {
    pub fn register_class() -> Result<WindowClass> {
        WindowClass::register(VISUALIZER_CLASS, ClassOptions::default())
    }

    /// Creates the widget hidden (revealed on the page's `vizReady`).
    /// `saved` = persisted `[x, y, w, h]` physical pixels from
    /// `widget_params[style].bounds`; `None` centres the default strip.
    /// `style_id` names the `Styles\Visualizer Widget\<id>.html` design file.
    pub fn open(
        class: &WindowClass,
        env: &WebEnvironment,
        saved: Option<[i32; 4]>,
        queue: CommandQueue,
        spectrum: Arc<RwLock<SpectrumSnapshot>>,
        style_id: &str,
    ) -> Result<Self> {
        let state = Rc::new(RefCell::new(HostState {
            comp: None,
            mouse_tracked: Cell::new(false),
            spectrum,
            posted_paused: Cell::new(false),
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
                        msg::WM_NCHITTEST => Some(frameless::widget_hit_test(hwnd, lparam)),
                        msg::WM_NCLBUTTONDBLCLK => Some(0),
                        msg::WM_EXITSIZEMOVE => {
                            let r = window::window_rect(hwnd);
                            queue.push(Command::SettingsMessage(format!(
                                "{{\"type\":\"vizMoved\",\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
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
                        msg::WM_TIMER => {
                            if wparam != TIMER_FEED {
                                return None;
                            }
                            let s = state.borrow();
                            let snap = s.spectrum.read().map(|g| *g).unwrap_or_default();
                            // Edge-triggered pause: one `paused:true` frame lets
                            // the page freeze; silent ticks while occluded keep
                            // the WebView at a hard 0 %.
                            if !snap.paused || !s.posted_paused.get() {
                                if let Some(c) = s.comp.as_ref() {
                                    c.post_json(&snap.json());
                                }
                                s.posted_paused.set(snap.paused);
                            }
                            Some(0)
                        }
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
                            menu.item(MENU_VIZ_REFRESH, "تحديث", false, false);
                            if menu.show_context(hwnd, x, y) == MENU_VIZ_REFRESH {
                                queue.push(Command::RefreshVisualizer);
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
        tracing::debug!(?hwnd, x, y, w, h, "visualizer widget host created");
        window.set_timer(TIMER_FEED, FEED_MS);

        let Some(document) = widget_doc::build_widget_document(
            coredesk_core::styles_fs::VISUALIZER_WIDGET_DIR,
            style_id,
        ) else {
            tracing::warn!(
                style_id,
                "visualizer design file missing — widget not spawned"
            );
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
                    if widget_doc::widget_message_allowed(&json, &["vizReady"]) {
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
    /// file. The fresh page's `vizReady` re-arms style/theme — `false`
    /// when the file can't be resolved.
    pub fn set_design(&self, style_id: &str) -> bool {
        if *self.state.borrow().design_id.borrow() == style_id {
            return true;
        }
        let Some(document) = widget_doc::build_widget_document(
            coredesk_core::styles_fs::VISUALIZER_WIDGET_DIR,
            style_id,
        ) else {
            tracing::warn!(style_id, "visualizer design file missing — swap ignored");
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

    /// A fresh page announced itself (`vizReady`): re-arm the paused edge so
    /// the next feed tick posts the current snapshot — a page that loads while
    /// the engine is slumbering must still see its one `paused:true` frame.
    pub fn note_document_ready(&self) {
        self.state.borrow().posted_paused.set(false);
    }

    /// Design swap moves the widget to that design's remembered rect — each
    /// `v1`…`v5` keeps its own spot (stored in `widget_params[id].bounds`).
    pub fn set_bounds(&self, rect: [i32; 4]) {
        let [x, y, w, h] = rect;
        if w > 0 && h > 0 {
            let _ = window::set_window_bounds(self.window.hwnd(), x, y, w, h);
        }
    }

    /// Re-asserts the WebView's transparent surface + DComp attach after a
    /// desktop composition-band rebuild (theme apply can strip the alpha).
    pub fn rearm_transparency(&self) {
        if let Some(c) = self.state.borrow().comp.as_ref() {
            c.rearm_transparency(
                window::dpi_of(self.window.hwnd()),
                self.window.client_size(),
            );
        }
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
/// monitor's DPI and centred on its work area. Themes without a remembered
/// `viz` rect land here.
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
