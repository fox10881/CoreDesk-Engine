//! Primary-taskbar geometry + visibility helpers for the native taskbar
//! resource widget — everything measured from the real `Shell_TrayWnd`,
//! never the work area (side app-bars deform it).
//!
//! [`covered`] is the taskbar-band sibling of
//! `wmf_wallpaper::should_pause_playback`: same foreground-window filters
//! (own PID, shell classes, transparent overlays exempt) but the cover
//! test runs against the *taskbar's* rect — a maximized window on a
//! bottom taskbar leaves the strip uncovered (and the widget visible).

use crate::bindings::{FindWindowExW, GetWindowRect, RECT};
use crate::{HWND, desktop, monitors};
use windows_core::PCWSTR;

/// The primary taskbar's window (`Shell_TrayWnd` is unique — secondaries
/// are `Shell_SecondaryTrayWnd`, unsupported by the widget for now).
pub fn primary_hwnd() -> Option<HWND> {
    let wide: Vec<u16> = "Shell_TrayWnd".encode_utf16().chain(Some(0)).collect();
    let hwnd = unsafe { FindWindowExW(None, None, PCWSTR(wide.as_ptr()), PCWSTR::null()) };
    (!hwnd.0.is_null()).then_some(hwnd)
}

/// The taskbar's screen rect — `None` while Explorer is mid-restart.
pub fn primary_rect() -> Option<RECT> {
    let hwnd = primary_hwnd()?;
    let mut rc = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rc) }
        .as_bool()
        .then_some(rc)
}

/// `true` while the taskbar is docked left/right — the native widget is
/// bottom-band only (same suppression the XAML styler applies: no sane
/// free strip exists on a vertical bar).
pub fn is_vertical() -> bool {
    primary_rect().is_some_and(|r| r.bottom - r.top > r.right - r.left)
}

/// The taskbar's visible slice on its own monitor — `None` when auto-hide
/// has retracted the bar off-screen (a retracted taskbar still reports a
/// rect, parked a few px past the monitor edge).
pub fn visible_rect() -> Option<RECT> {
    let bar = primary_rect()?;
    let mon = monitors::monitor_from_point(
        bar.left + (bar.right - bar.left) / 2,
        bar.top + (bar.bottom - bar.top) / 2,
    );
    let info = monitors::query(mon)?;
    let vis = RECT {
        left: bar.left.max(info.bounds.left),
        top: bar.top.max(info.bounds.top),
        right: bar.right.min(info.bounds.right),
        bottom: bar.bottom.min(info.bounds.bottom),
    };
    // Under ~8 px of the band showing, the strip is retracted — treat as
    // hidden rather than draw a sliver of widget.
    (vis.bottom - vis.top >= 8 && vis.right - vis.left >= 8).then_some(vis)
}

/// `true` when a foreign window covers the taskbar band — a fullscreen
/// app/game the topmost widget must not float above. Same exemptions as
/// `should_pause_playback` (our own process, shell surfaces, transparent
/// overlays) plus the widget window itself (`ours` HWND).
pub fn covered(bar: RECT, ours: HWND) -> bool {
    let fg = desktop::root_ancestor(desktop::foreground_window());
    if fg.0.is_null() || fg.0 == ours.0 || !desktop::has_visible_surface(fg) {
        return false;
    }
    let (our_pid, shell_pid) = (
        std::process::id(),
        desktop::window_pid(desktop::shell_window()),
    );
    let fg_pid = desktop::window_pid(fg);
    if fg_pid == our_pid || fg_pid == shell_pid {
        return false;
    }
    if matches!(
        desktop::class_name(fg).as_str(),
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "XamlExplorerHostIslandWindow"
            | "CEF-OSC-WIDGET"
    ) {
        return false;
    }
    const EX: u32 = 0x0000_0080 | 0x0800_0000 | 0x0000_0020; // TRANSPARENT | NOACTIVATE | TOOLWINDOW
    if desktop::ex_style(fg) & EX != 0 {
        return false;
    }
    let mut r = RECT::default();
    if !unsafe { GetWindowRect(crate::bindings::HWND(fg.0), &mut r) }.as_bool() {
        return false;
    }
    // The band counts as covered when the foreign rect swallows it whole
    // (4 px tolerance for rounded fullscreen edges).
    r.left <= bar.left + 4
        && r.top <= bar.top + 4
        && r.right >= bar.right - 4
        && r.bottom >= bar.bottom - 4
}
