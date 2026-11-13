//! Host-side controller for the Start Menu styler — the native replacement
//! for the Windhawk `windows-11-start-menu-styler` mod.
//!
//! `coredesk_tap.dll` is injected into `StartMenuExperienceHost.exe` by a
//! `CreateRemoteThread(LoadLibraryW)` — the taskbar's `WH_CALLWNDPROC` vector
//! can't reach SMEH: it keeps **no windows at all** until the menu first
//! opens (31 threads, zero `HWND`s — verified via TH32CS_SNAPTHREAD +
//! EnumThreadWindows). Once resident, the DLL runs the styling engine itself:
//! `WM_COPYDATA` delivers the serialized theme
//! (`coredesk_core::start_menu_themes::theme_wire`) to the injected
//! command window, which styles the menu's XAML tree in place.
//!
//! `StartMenuExperienceHost.exe` is spawned lazily by the shell and can
//! idle-exit any time, so `apply_start_menu_theme` stores the selected
//! style and `ensure()` (driven by the app's periodic tick) re-injects and
//! re-sends it whenever the host process restarts.

use crate::bindings::{HWND, LPARAM, SendMessageTimeoutW, WPARAM};
use crate::wide::from_wide;
use std::sync::Mutex;
use windows_core::{BOOL, PCSTR, PCWSTR};

windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject: *mut core::ffi::c_void) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn CreateRemoteThread(process: *mut core::ffi::c_void, attrs: *const core::ffi::c_void, stack: usize, start: *mut core::ffi::c_void, param: *const core::ffi::c_void, flags: u32, tid: *mut u32) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn CreateToolhelp32Snapshot(dwflags: u32, th32processid: u32) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GetExitCodeThread(thread: HANDLE, code: *mut u32) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule: *mut core::ffi::c_void, lpprocname: PCSTR) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(name: PCWSTR) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn OpenProcess(dwdesiredaccess: u32, binherithandle: BOOL, dwprocessid: u32) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn Process32FirstW(hsnapshot: *mut core::ffi::c_void, lppe: *mut core::ffi::c_void) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn Process32NextW(hsnapshot: *mut core::ffi::c_void, lppe: *mut core::ffi::c_void) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn Sleep(dwmilliseconds: u32));
windows_core::link!("kernel32.dll" "system" fn VirtualAllocEx(process: HANDLE, addr: *const core::ffi::c_void, size: usize, alloctype: u32, protect: u32) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn VirtualFreeEx(process: HANDLE, addr: *mut core::ffi::c_void, size: usize, freetype: u32) -> BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(handle: HANDLE, ms: u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn WriteProcessMemory(process: HANDLE, addr: *mut core::ffi::c_void, buf: *const core::ffi::c_void, size: usize, written: *mut usize) -> BOOL);
windows_core::link!("user32.dll" "system" fn IsWindow(hwnd: HWND) -> BOOL);
windows_core::link!("user32.dll" "system" fn RegisterWindowMessageW(lpstring: PCWSTR) -> u32);

use crate::bindings::HANDLE;

const SMTO_ABORTIFHUNG: u32 = 0x0002;
const SMTO_BLOCK: u32 = 0x0001;
const SMTO_ERRORONEXIT: u32 = 0x0020;
const WM_COPYDATA: u32 = 0x004A;
/// `COPYDATASTRUCT.dwData` values the injected DLL understands.
const CDS_THEME: usize = u32::from_be_bytes(*b"SMST") as usize;
const CDS_RESET: usize = u32::from_be_bytes(*b"SMSR") as usize;
/// Announces the host pid so the DLL can restore styles if coredesk dies.
const CDS_HELLO: usize = u32::from_be_bytes(*b"SMHI") as usize;

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_CREATE_THREAD: u32 = 0x0002;
const PROCESS_VM_OPERATION: u32 = 0x0008;
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_VM_WRITE: u32 = 0x0020;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READWRITE: u32 = 0x04;

/// `PROCESSENTRY32W` (the fields we read — `szExeFile` tail is MAX_PATH).
#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent_pid: u32,
    base_pri: i32,
    flags: u32,
    exe_file: [u16; 260],
}

#[repr(C)]
struct CopyDataStruct {
    dw_data: usize,
    cb_data: u32,
    lp_data: *const core::ffi::c_void,
}

/// Per-process injection bookkeeping — one per host the `CDES-SMS1` wire
/// is delivered to: `StartMenuExperienceHost.exe` for the menu chrome and
/// `SearchHost.exe` for the search-results WebView2 (which lives there on
/// current builds).
struct Host {
    /// Image name matched in the toolhelp snapshot.
    exe: &'static str,
    /// `true` when the host runs AppContainer and needs the `%ProgramData%`
    /// ACE'd resident copy (`SearchHost` — same restriction as SEH).
    appcontainer: bool,
    /// The in-host command window (`CoreDeskTAPSM` class, owned by `pid`).
    cmd_hwnd: HWND,
    /// Pid the DLL is loaded in.
    pid: u32,
    /// `RegisterWindowMessage` id for the unload command.
    msg_unload: u32,
    /// Whether `CDS_HELLO` was delivered for `pid` (arms the DLL's host
    /// death-watch once per host lifetime).
    hello_sent: bool,
}
unsafe impl Send for Host {}

impl Host {
    fn new(exe: &'static str, appcontainer: bool) -> Self {
        Self {
            exe,
            appcontainer,
            cmd_hwnd: HWND(core::ptr::null_mut()),
            pid: 0,
            msg_unload: 0,
            hello_sent: false,
        }
    }
}

struct Sms {
    /// The catalog id last sent ("" = Windows Default).
    style: String,
    /// The theme serialized for transport — cached so a re-inject after a
    /// host restart doesn't rebuild it.
    wire: String,
    /// The DIP height cap `wire` was serialized with — a display-scale or
    /// work-area change drifts it and `ensure()` re-serializes.
    fit: f64,
    /// `[SMEH, SearchHost]` — both spawned lazily by the shell.
    hosts: [Host; 2],
}
unsafe impl Send for Sms {}

static SMS: Mutex<Option<Sms>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Sms>> {
    SMS.lock().unwrap_or_else(|p| p.into_inner())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// Pid of `exe`, or `None` when the shell hasn't spawned it yet (both
/// Start-Menu hosts idle out and respawn on demand).
fn find_pid(exe: &str) -> Option<u32> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap.is_null() {
        return None;
    }
    let mut pe = ProcessEntry {
        size: core::mem::size_of::<ProcessEntry>() as u32,
        usage: 0,
        pid: 0,
        heap: 0,
        module: 0,
        threads: 0,
        parent_pid: 0,
        base_pri: 0,
        flags: 0,
        exe_file: [0; 260],
    };
    let mut found = None;
    let mut cur = unsafe { Process32FirstW(snap, (&mut pe as *mut ProcessEntry).cast()) };
    while cur.0 != 0 {
        let len = pe
            .exe_file
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(pe.exe_file.len());
        if from_wide(&pe.exe_file[..len]).eq_ignore_ascii_case(exe) {
            found = Some(pe.pid);
            break;
        }
        cur = unsafe { Process32NextW(snap, (&mut pe as *mut ProcessEntry).cast()) };
    }
    unsafe {
        let _ = CloseHandle(snap);
    }
    found
}

/// SMEH's `CoreDeskTAPSM` command window (a message-only window —
/// `EnumWindows` can't enumerate these; the versioned-class enumeration in
/// `tap` resolves them and prefers the newest protocol build). The
/// distinct class family keeps it unambiguous with explorer's
/// `CoreDeskTAP`.
fn find_cmd_window(pid: u32) -> Option<HWND> {
    crate::tap::resident_windows_matching(pid, "CoreDeskTAPSM")
        .into_iter()
        .next()
}

/// `WM_COPYDATA` payload send — waits up to 3 s for the injected DLL to
/// consume the serialized theme.
fn send_copydata(hwnd: HWND, tag: usize, payload: &[u8]) -> bool {
    let cds = CopyDataStruct {
        dw_data: tag,
        cb_data: payload.len() as u32,
        lp_data: payload.as_ptr() as *const core::ffi::c_void,
    };
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&cds as *const CopyDataStruct as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
            3_000,
            Some(&mut result),
        )
        .0 != 0
    }
}

/// Maps `coredesk_tap.dll` into the host via `CreateRemoteThread(LoadLibraryW)`,
/// then waits for its command window. `kernel32`'s image base is shared
/// per-boot, so the local `LoadLibraryW` address is the remote one.
fn inject(host: &mut Host, pid: u32) -> bool {
    // Unique-per-build module name — a stale pinned resident can't be
    // evicted by re-injecting the same path (LoadLibrary dedups), so the
    // versioned copy is the only way a fresh resident co-loads.
    let path = match if host.appcontainer {
        crate::tap::tap_injectable_path_appcontainer()
    } else {
        crate::tap::tap_injectable_path()
    } {
        Some(p) if p.exists() => p,
        _ => return false,
    };
    let wide_path = wide(&path.to_string_lossy());
    let k32 = unsafe { GetModuleHandleW(PCWSTR(wide("kernel32.dll").as_ptr())) };
    if k32.is_null() {
        return false;
    }
    let load = unsafe { GetProcAddress(k32, PCSTR(c"LoadLibraryW".as_ptr() as *const u8)) };
    if load.is_null() {
        return false;
    }
    let proc = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_READ
                | PROCESS_VM_WRITE
                | PROCESS_QUERY_INFORMATION,
            BOOL(0),
            pid,
        )
    };
    if proc.0.is_null() {
        return false;
    }
    let ok = unsafe {
        let bytes = wide_path.len() * 2;
        let mem = VirtualAllocEx(
            proc,
            core::ptr::null(),
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        let mut wrote = 0usize;
        if mem.is_null()
            || WriteProcessMemory(
                proc,
                mem,
                wide_path.as_ptr() as *const core::ffi::c_void,
                bytes,
                &mut wrote,
            )
            .0 == 0
            || wrote != bytes
        {
            let _ = CloseHandle(proc.0);
            return false;
        }
        let thread = CreateRemoteThread(
            proc.0,
            core::ptr::null(),
            0,
            load,
            mem,
            0,
            core::ptr::null_mut(),
        );
        if thread.0.is_null() {
            let _ = VirtualFreeEx(proc, mem, 0, MEM_RELEASE);
            let _ = CloseHandle(proc.0);
            return false;
        }
        // The remote thread exits when LoadLibrary returns — bounded wait.
        let _ = WaitForSingleObject(thread, 5_000);
        let mut code = 0u32;
        let _ = GetExitCodeThread(thread, &mut code);
        let _ = CloseHandle(thread.0);
        let _ = VirtualFreeEx(proc, mem, 0, MEM_RELEASE);
        let _ = CloseHandle(proc.0);
        code != 0 // exit code = the remote HMODULE
    };
    if !ok {
        return false;
    }
    host.msg_unload =
        unsafe { RegisterWindowMessageW(PCWSTR(wide("CoreDeskEngine.TAP.Unload").as_ptr())) };
    // Up to 5 s for the DLL to boot its command window inside the host.
    for _ in 0..100 {
        if let Some(cmd) = find_cmd_window(pid) {
            host.cmd_hwnd = cmd;
            host.pid = pid;
            // One resident per process — unwind any stale co-loaded build.
            crate::tap::unload_foreign_residents_matching(pid, cmd, "CoreDeskTAPSM");
            return true;
        }
        unsafe { Sleep(50) };
    }
    false
}

fn unload_locked(host: &mut Host) {
    if !host.cmd_hwnd.0.is_null() && host.msg_unload != 0 {
        let _ = send_copydata(host.cmd_hwnd, CDS_RESET, &[]);
        // Registered unload message — the DLL restores before returning.
        let mut result = 0usize;
        unsafe {
            let _ = SendMessageTimeoutW(
                host.cmd_hwnd,
                host.msg_unload,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
                3_000,
                Some(&mut result),
            );
        }
    }
    host.cmd_hwnd = HWND(core::ptr::null_mut());
    host.pid = 0;
    host.hello_sent = false;
}

/// Sends the stored wire to the resident DLL; re-injects when the host
/// restarted under us.
fn deliver_locked(host: &mut Host, wire: &str) -> bool {
    if wire.is_empty() {
        return send_copydata(host.cmd_hwnd, CDS_RESET, &[]);
    }
    // Arm the DLL's host death-watch once per host lifetime — styles revert
    // to stock if coredesk dies. A `Build`-less same-class window is this
    // build's resident left inert by a prior shutdown — the revive hello
    // must re-send even on a repeat delivery to the same hwnd.
    if !host.hello_sent || crate::tap::resident_version(host.cmd_hwnd) == 0 {
        let pid = std::process::id();
        if send_copydata(host.cmd_hwnd, CDS_HELLO, &pid.to_ne_bytes()) {
            host.hello_sent = true;
        }
    }
    send_copydata(host.cmd_hwnd, CDS_THEME, wire.as_bytes())
}

/// DIP headroom the Start menu can occupy: each monitor's work-area height
/// in device-independent units (physical px ÷ scale), the smallest wins —
/// SMEH can surface on any of them. XAML sizes don't scale with DPI, so a
/// theme authored on a high-DIP desktop overflows a scaled-down screen and
/// its bottom chrome (user tile, power) clips out of view; `theme_wire`
/// rewrites oversized `Height`/`MaxHeight` rules down to this cap.
///
/// The reserve covers what sits *below* `Grid#MainMenu`: the navigation
/// strip (`NavigationPaneView` — user tile + power, ~56 DIP plus the
/// translate themes give it) is a sibling, not a child, so it stacks onto
/// the frame's total height — and a small margin keeps the menu off the
/// work-area edge. `f64::INFINITY` = unknown.
fn start_fit_height() -> f64 {
    const EDGE_MARGIN_DIP: f64 = 16.0;
    const BOTTOM_CHROME_DIP: f64 = 72.0;
    crate::monitors::enumerate()
        .iter()
        .filter_map(|m| {
            let scale = m.scale() as f64;
            (scale > 0.0).then(|| {
                (m.work_area.bottom - m.work_area.top) as f64 / scale
                    - EDGE_MARGIN_DIP
                    - BOTTOM_CHROME_DIP
            })
        })
        .fold(f64::INFINITY, f64::min)
}

/// Applies the selected Start Menu style.
///
/// `id` is a catalog id (`""` = Windows Default). Returns `false` only when
/// a running SMEH rejected the command channel — when SMEH isn't running at
/// all the selection is stored and applied on its next launch, so the call
/// still reports success (same UX as the old Windhawk write).
pub fn apply_start_menu_theme(id: &str) -> bool {
    let fit = start_fit_height();
    let wire = if id.is_empty() {
        String::new() // empty wire → CDS_RESET
    } else {
        match coredesk_core::start_menu_themes::theme_wire(id, fit.is_finite().then_some(fit)) {
            Some(w) => w,
            None => return false,
        }
    };
    let mut guard = lock();
    let sms = guard.get_or_insert_with(|| Sms {
        style: String::new(),
        wire: String::new(),
        fit: f64::INFINITY,
        hosts: [
            Host::new("startmenuexperiencehost.exe", false),
            Host::new("searchhost.exe", true),
        ],
    });
    sms.style = id.to_string();
    sms.wire = wire;
    sms.fit = fit;
    let mut ok = true;
    for host in &mut sms.hosts {
        let Some(pid) = find_pid(host.exe) else {
            continue; // applies when the host next launches
        };
        let stale = host.pid != pid || unsafe { IsWindow(host.cmd_hwnd) }.0 == 0;
        if stale {
            host.cmd_hwnd = find_cmd_window(pid).unwrap_or(HWND(core::ptr::null_mut()));
            host.hello_sent = false;
            if host.cmd_hwnd.0.is_null() {
                // Fresh (or restarted) host — map the DLL in.
                host.pid = 0;
                if !inject(host, pid) {
                    ok = false;
                    continue;
                }
            } else {
                host.pid = pid;
            }
        }
        ok &= deliver_locked(host, &sms.wire);
    }
    ok
}

/// Cheap liveness check driven by the app's periodic tick: re-injects and
/// re-delivers the selected style when either host restarted.
pub fn ensure() {
    // Anything deliverable counts — a tint-only wire keeps the host armed
    // even with no style selected.
    let needs = lock().as_ref().map(|s| !s.wire.is_empty()).unwrap_or(false);
    if !needs {
        return;
    }
    let mut guard = lock();
    let Some(sms) = guard.as_mut() else {
        return;
    };
    let mut any_stale = false;
    for host in &sms.hosts {
        if find_pid(host.exe).is_none() {
            continue;
        }
        let stale = host.pid == 0 || unsafe { IsWindow(host.cmd_hwnd) }.0 == 0;
        let stale = stale
            // Dormant resident (`unload()` stripped its `Build` stamp) —
            // deliver_locked re-sends the revive hello, so treat as stale.
            || crate::tap::resident_version(host.cmd_hwnd) == 0
            || {
            // `IsWindow` on a dead-process window fails — also guard the pid.
            let alive = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, BOOL(0), host.pid) };
            if !alive.0.is_null() {
                unsafe {
                    let _ = CloseHandle(alive.0);
                }
                false
            } else {
                true
            }
        };
        any_stale |= stale;
    }
    // A scale/work-area change drifts the baked height cap even while the
    // hosts stay healthy — re-serialize so the menu refits the new DIP space.
    let fit_drifted = (start_fit_height() - sms.fit).abs() > 0.5;
    if !any_stale && !fit_drifted {
        return;
    }
    let style = sms.style.clone();
    drop(guard);
    let _ = apply_start_menu_theme(&style);
}

/// Call on shutdown: restores the Start Menu and unloads the DLL wherever
/// it was injected.
pub fn shutdown() {
    let mut guard = lock();
    if let Some(mut sms) = guard.take() {
        for host in &mut sms.hosts {
            unload_locked(host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnoses the live injection path against the running SMEH — prints
    /// the pid and command-window discovery results; does not apply a theme
    /// (run `apply` through the settings CLI for that).
    #[test]
    #[ignore]
    fn probe_smeh() {
        let pid = find_pid("startmenuexperiencehost.exe");
        eprintln!("find_pid -> {pid:?}");
        if let Some(pid) = pid {
            eprintln!("cmd_window -> {:?}", find_cmd_window(pid).map(|h| h.0));
        }
    }

    /// Full apply — injects + delivers TranslucentStartMenu to a live SMEH.
    #[test]
    #[ignore]
    fn probe_apply() {
        let ok = apply_start_menu_theme("GTA VI");
        eprintln!("apply -> {ok}");
        let g = lock();
        if let Some(s) = g.as_ref() {
            for h in &s.hosts {
                eprintln!("host pid={} cmd={:?}", h.pid, h.cmd_hwnd.0);
            }
        }
    }
}
