//! `coredesk_tap.dll` — the resident XAML-styling engine injected into
//! `explorer.exe` (taskbar themes), `StartMenuExperienceHost.exe` (Start
//! Menu themes) and `ShellExperienceHost.exe`/`ShellHost.exe` (Notification
//! Center themes), modelled on TranslucentTB's `ExplorerTAP` + Windhawk's
//! `visualtreewatcher`.
//!
//! Injected by `coredesk.exe` through a `WH_CALLWNDPROC` hook (the
//! `CallWndProc` export is the injection vector). On attach it:
//!  1. opens a message-only command window (`CoreDeskTAP` /
//!     `CoreDeskTAPSM`) the host delivers `CDES-TBS1`/`CDES-SMS1` theme
//!     wires to over `WM_COPYDATA`;
//!  2. initializes XAML Diagnostics (`InitializeXamlDiagnosticsEx`) so a
//!     `TAPSite` COM object hands us the visual-tree service that feeds
//!     `styler`;
//!  3. restores every original property value when the host exits, resets,
//!     or sends `CoreDeskEngine.TAP.Unload`.

#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod bindings;
mod command;
#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod ebm;
mod effects;
#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod fx_bindings;
#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod mux_bindings;
mod shared;
mod site;
#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod styler;
#[allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
mod styler_bindings;
mod webview;
mod win32;

/// Debug-build-only tracing: the DLL swallows COM errors by design (Explorer
/// must never crash for cosmetics), so diagnosis needs a side channel. Each
/// line appends to `%TEMP%\coredesk_tap.log`; release builds compile it out.
/// One shared `File` under a mutex — per-line `open()` raced concurrent
/// writers and could silently drop lines.
#[cfg(debug_assertions)]
pub(crate) fn log(msg: &str) {
    use std::io::Write;
    static LOG_FILE: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);
    let mut guard = LOG_FILE.lock().unwrap_or_else(|p| p.into_inner());
    if guard.is_none() {
        *guard = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::env::temp_dir().join("coredesk_tap.log"))
            .ok();
    }
    if let Some(f) = guard.as_mut() {
        let _ = writeln!(f, "{:>8.3?} {msg}", std::time::Instant::now());
        let _ = f.flush();
    }
}

#[cfg(not(debug_assertions))]
#[inline(always)]
pub(crate) fn log(_msg: &str) {}

macro_rules! tap_log {
    ($($t:tt)*) => { $crate::log(&format!($($t)*)) };
}
pub(crate) use tap_log;

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

static DLL_INSTANCE: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

/// A bootstrap spawn is in flight — dedups concurrent `ensure_boot`
/// nudges landing while the spawned thread is still queueing
/// `command::run`/`site::install`.
static BOOTING: AtomicBool = AtomicBool::new(false);

/// `true` only in processes where the resident is allowed to run — the
/// shell hosts DllMain boots on. The exported hook/winevent procs call
/// `ensure_boot` unconditionally, and the app itself `LoadLibrary`s this
/// image for `SetWindowsHookExW`/`SetWinEventHook`: without this gate a
/// proc firing in `coredesk.exe` (a hook landing on our own thread, or an
/// in-context win-event) would spawn `command::run`/`site::install`
/// inside the app — which never self-pins, so the app's `FreeLibrary`
/// unmaps the image out from under the live threads (`0xc0000005` at a
/// `coredesk_tap_*_unloaded` address — the observed app crash).
fn is_host_process() -> bool {
    static IS_HOST: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_HOST.get_or_init(|| {
        is_explorer_process()
            || is_smeh_process()
            || is_nch_process()
            || is_searchhost_process()
    })
}

/// Bootstrap retry bounded by host liveness. The exported hook procs run
/// inside the target only while it pumps messages, which is exactly when a
/// resident whose `bootstrap` starved mid-attach (PLM suspension racing
/// `DllMain`'s thread spawn, or a `command::run` that returned early) can
/// finish booting — a mapped-but-windowless resident otherwise stays a
/// zombie forever since `LoadLibrary` dedups every later inject. `DllMain`
/// funnels through it too so first-attach and revive share the dedup; an
/// inert (unloaded) resident never re-boots.
fn ensure_boot() {
    if !is_host_process()
        || crate::command::is_unloaded()
        || crate::command::pump_running()
        || BOOTING.swap(true, Ordering::AcqRel)
    {
        return;
    }
    std::thread::spawn(|| {
        let _ = std::panic::catch_unwind(core::panic::AssertUnwindSafe(bootstrap));
        BOOTING.store(false, Ordering::Release);
    });
}

pub(crate) fn dll_instance() -> win32::HMODULE {
    DLL_INSTANCE.load(Ordering::Relaxed)
}

/// # Safety
/// Entry point invoked by the loader; `reason` values come from Windows.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(
    hinstance: win32::HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    if reason == win32::DLL_PROCESS_ATTACH {
        DLL_INSTANCE.store(hinstance, Ordering::Relaxed);
        unsafe {
            let _ = win32::DisableThreadLibraryCalls(hinstance);
        }
        if is_host_process() {
            // Self-pin: the host unhooks right after our command window
            // appears, and `InitializeXamlDiagnosticsEx` only pins us later,
            // inside `SetSite`. Without our own pin the gap between
            // `UnhookWindowsHookEx` and `SetSite` lets the refcount hit zero
            // — the resident unloaded ~40ms after every inject and the whole
            // cycle repeated each host tick. The inert-resident design makes
            // a permanent pin correct anyway.
            const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x4;
            const GET_MODULE_HANDLE_EX_FLAG_PIN: u32 = 0x1;
            let mut pinned = core::ptr::null_mut();
            unsafe {
                let _ = win32::GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                    windows_core::PCWSTR(hinstance as *const u16),
                    &mut pinned,
                );
            }
            tap_log!("DllMain attach hmod={hinstance:?} smeh={}", is_smeh());
            ensure_boot();
        }
    } else if reason == 0
        && (is_explorer_process()
            || is_smeh_process()
            || is_nch_process()
            || is_searchhost_process())
    {
        // DLL_PROCESS_DETACH — only meaningful inside a host; the host-side
        // `FreeLibrary(tap.lib)` in coredesk.exe would otherwise log the
        // same line and look like a resident unload.
        tap_log!("DllMain DETACH smeh={} nch={}", is_smeh(), is_nch_process());
    }
    1
}

/// The exported hook procedure — presence in this DLL is all the injection
/// needs. It fires only while the hooked thread retrieves messages, which
/// doubles as the liveness signal that retries a bootstrap starved
/// mid-attach: two atomic loads per message when already booted.
///
/// # Safety
/// Called by Windows from the hooked thread with valid arguments.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn CallWndProc(
    code: i32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    ensure_boot();
    unsafe { win32::CallNextHookEx(core::ptr::null_mut(), code, wparam, lparam) }
}

/// The exported win-event procedure — the `WINEVENT_INCONTEXT` injection
/// vector for suspended AppContainer hosts: the OS maps this DLL into the
/// target when it fires its first accessibility event (i.e. exactly when a
/// frozen host wakes to render). Every later event is another liveness
/// signal — the same revive nudge `CallWndProc` gives message-pumping
/// hosts — so a bootstrap that starved mid-attach retries here.
///
/// # Safety
/// Called by Windows inside the hooked process with valid arguments.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn WinEventProc(
    _hook: *mut core::ffi::c_void,
    _event: u32,
    _hwnd: win32::HWND,
    _object: i32,
    _child: i32,
    _tid: u32,
    _time: u32,
) {
    ensure_boot();
}

fn process_name_is(target: &str) -> bool {
    let mut path = [0u16; 260];
    let len = unsafe {
        win32::GetModuleFileNameW(
            core::ptr::null_mut(),
            windows_core::PWSTR(path.as_mut_ptr()),
            path.len() as u32,
        )
    };
    if len == 0 {
        return false;
    }
    let name = &path[..len as usize];
    let want: Vec<u16> = target.encode_utf16().collect();
    // `|0x20` lowercases ASCII letters — enough for a suffix compare.
    name.len() >= want.len()
        && name[name.len() - want.len()..]
            .iter()
            .zip(&want)
            .all(|(a, b)| (*a | 0x20) == *b)
}

fn is_explorer_process() -> bool {
    process_name_is("explorer.exe")
}

/// `true` inside `StartMenuExperienceHost.exe` — the Start Menu styler's
/// host process. Checked once; the process image never changes.
pub(crate) fn is_smeh() -> bool {
    static IS_SMEH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_SMEH.get_or_init(|| process_name_is("startmenuexperiencehost.exe"))
}

fn is_smeh_process() -> bool {
    is_smeh()
}

/// `true` inside `SearchHost.exe` — the search-results host (on current
/// builds the Start Menu's search WebView2 lives here rather than SMEH).
/// It receives the same `CDES-SMS1` wire under the `CoreDeskTAPSM` command
/// window, but SMEH-only machinery (the layout detour + exit throttle)
/// stays gated on [`is_smeh`].
pub(crate) fn is_searchhost() -> bool {
    static IS_SH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_SH.get_or_init(|| process_name_is("searchhost.exe"))
}

fn is_searchhost_process() -> bool {
    is_searchhost()
}

/// `true` inside `ShellExperienceHost.exe` or `ShellHost.exe` — the
/// Notification Center styler's host processes.
pub(crate) fn is_nch() -> bool {
    static IS_NCH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_NCH.get_or_init(|| {
        process_name_is("shellexperiencehost.exe") || process_name_is("shellhost.exe")
    })
}

fn is_nch_process() -> bool {
    is_nch()
}

fn bootstrap() {
    // MTA for the threads we own; the XAML callbacks arrive on the UI thread
    // the diagnostics service marshals to, so this only affects our own calls.
    unsafe {
        let _ = win32::RoInitialize(0);
    }
    std::thread::spawn(command::run);
    std::thread::spawn(site::install);
}
