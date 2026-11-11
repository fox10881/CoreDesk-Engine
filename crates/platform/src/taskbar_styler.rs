//! Host-side controller for the taskbar XAML styler — the native
//! replacement for the Windhawk `windows-11-taskbar-styler` mod.
//!
//! `tap::ensure_resident` guarantees `coredesk_tap.dll` is resident in
//! `explorer.exe` with its command window up, then `WM_COPYDATA` delivers
//! the serialized theme (`coredesk_core::taskbar_themes::theme_wire`, a
//! `CDES-TBS1` wire) which the injected styler applies to the taskbar's
//! XAML tree in place. The DLL's death watch restores stock styling if
//! coredesk.exe dies.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use windows_core::{PCSTR, PCWSTR};

windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(name: PCWSTR) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule: *mut core::ffi::c_void, lpprocname: PCSTR) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn OpenProcess(dwdesiredaccess: u32, binherithandle: windows_core::BOOL, dwprocessid: u32) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GetExitCodeProcess(hprocess: *mut core::ffi::c_void, lpexitcode: *mut u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(hhandle: *mut core::ffi::c_void, dwmilliseconds: u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject: *mut core::ffi::c_void) -> windows_core::BOOL);
windows_core::link!("advapi32.dll" "system" fn RegCreateKeyExW(hkey: isize, lpsubkey: PCWSTR, reserved: u32, lpclass: PCWSTR, dwoptions: u32, samdesired: u32, lpsecurityattributes: *const core::ffi::c_void, phkresult: *mut isize, lpdwdisposition: *mut u32) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegSetValueExW(hkey: isize, lpvaluename: PCWSTR, reserved: u32, dwtype: u32, lpdata: *const u8, cbdata: u32) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegGetValueW(hkey: isize, lpsubkey: PCWSTR, lpvalue: PCWSTR, dwflags: u32, pdwtype: *mut u32, pvdata: *mut core::ffi::c_void, pcbdata: *mut u32) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegDeleteValueW(hkey: isize, lpvaluename: PCWSTR) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegCloseKey(hkey: isize) -> i32);

/// `COPYDATASTRUCT.dwData` values — must match `crates/tap/src/command.rs`.
const CDS_TB_THEME: usize = u32::from_be_bytes(*b"TBST") as usize;
const CDS_TB_RESET: usize = u32::from_be_bytes(*b"TBSR") as usize;
const CDS_TB_HELLO: usize = u32::from_be_bytes(*b"TBHI") as usize;

struct Tb {
    /// The catalog id last delivered ("" = Windows Default — nothing armed).
    style: String,
    /// The wire last built — cached so `ensure` re-delivers after an
    /// explorer restart without re-serializing the catalog.
    wire: String,
    /// The resident command window the wire was delivered to (`HWND` as
    /// `usize` for `Send`) — resident identity, not `explorer_pid`: after a
    /// restart the `fresh` flag may already be consumed by another caller on
    /// this tick, so the hwnd is the only reliable "this resident never got
    /// the theme" signal.
    cmd_hwnd: usize,
    /// Whether `CDS_TB_HELLO` reached the current DLL residency (the death
    /// watch arms once per injection — a fresh resident needs it again).
    hello_sent: bool,
    /// The armed theme is intentionally withheld while the taskbar is
    /// vertically docked (left/right). Win11's taskbar has no vertical XAML
    /// mode; on builds that force one the layout churns without converging,
    /// and our property writes inside that churn freeze the shell UI thread
    /// — the taskbar never renders and explorer's input loop starves.
    suppressed: bool,
}

static TB: Mutex<Option<Tb>> = Mutex::new(None);
/// Whether a taskbar theme is armed — read lock-free so callers never race
/// the TB mutex order in `reset`.
static ARMED: AtomicBool = AtomicBool::new(false);

/// `true` while a taskbar style is armed — `tap` keeps the DLL resident.
pub fn is_armed() -> bool {
    ARMED.load(Ordering::Acquire)
}

fn lock() -> std::sync::MutexGuard<'static, Option<Tb>> {
    TB.lock().unwrap_or_else(|p| p.into_inner())
}

// ---- `RtlQueryFeatureConfiguration` (Squircle's weather-variant probe) ----

#[repr(C)]
struct RtlFeatureConfiguration {
    feature_id: u32,
    /// `group:4 | enabledState:2 | enabledStateOptions:1 | unused1:1 |
    /// variant:6 | variantPayloadKind:2 | unused2:16`
    flags: u32,
    payload: u32,
}

type RtlQueryFn =
    unsafe extern "system" fn(u32, i32, *mut i64, *mut RtlFeatureConfiguration) -> i32;

/// `IsOsFeatureEnabled` — enabledState 2 = Enabled (1 = Disabled, 0 =
/// Default). `false` when the query fails — the base payload then applies.
fn feature_enabled(feature_id: u32) -> bool {
    let ntdll = unsafe { GetModuleHandleW(PCWSTR(widestr("ntdll.dll").as_ptr())) };
    if ntdll.is_null() {
        return false;
    }
    let f = unsafe {
        GetProcAddress(
            ntdll,
            PCSTR(c"RtlQueryFeatureConfiguration".as_ptr() as *const u8),
        )
    };
    if f.is_null() {
        return false;
    }
    let f: RtlQueryFn = unsafe { core::mem::transmute(f) };
    let mut cfg = RtlFeatureConfiguration {
        feature_id: 0,
        flags: 0,
        payload: 0,
    };
    let mut stamp = 0i64;
    if unsafe { f(feature_id, 1, &mut stamp, &mut cfg) } < 0 {
        return false;
    }
    (cfg.flags >> 4) & 0b11 == 2
}

fn widestr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// `true` when any taskbar window is taller than wide — docked left or
/// right. Measured from the real `Shell_TrayWnd`/`Shell_SecondaryTrayWnd`
/// rects (not the work area, which side app-bars also deform). While this
/// holds the styler must not deliver a theme: the vertical XAML layout
/// never converges, and each property write inside the visual-tree churn
/// keeps explorer's UI thread saturated.
fn taskbar_is_vertical() -> bool {
    use crate::bindings::{FindWindowExW, GetWindowRect};
    for class in ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"] {
        let wide = widestr(class);
        let mut hwnd = unsafe { FindWindowExW(None, None, PCWSTR(wide.as_ptr()), PCWSTR::null()) };
        while !hwnd.0.is_null() {
            let mut rc = crate::bindings::RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rc) }.as_bool()
                && rc.bottom - rc.top > rc.right - rc.left
            {
                return true;
            }
            hwnd =
                unsafe { FindWindowExW(None, Some(hwnd), PCWSTR(wide.as_ptr()), PCWSTR::null()) };
        }
    }
    false
}

/// Sends the stored wire to the resident DLL. A resident this session never
/// delivered to (hwnd differs) gets the hello re-armed first — the wire
/// itself is (re)sent by the caller's decision.
fn deliver(hwnd: crate::bindings::HWND, tb: &mut Tb) -> bool {
    // A same-class window with no `Build` stamp is this build's resident
    // left inert by a prior app's shutdown — the revive hello must precede
    // the wire even when the hwnd matches a previously-delivered one.
    if tb.cmd_hwnd != hwnd.0 as usize || crate::tap::resident_version(hwnd) == 0 {
        tb.hello_sent = false;
    }
    if !tb.hello_sent {
        let pid = std::process::id();
        if crate::tap::send_copydata(hwnd, CDS_TB_HELLO, &pid.to_ne_bytes()) {
            tb.hello_sent = true;
        }
    }
    if crate::tap::send_copydata(hwnd, CDS_TB_THEME, tb.wire.as_bytes()) {
        tb.cmd_hwnd = hwnd.0 as usize;
        true
    } else {
        false
    }
}

/// Applies the selected taskbar style. `id` is a catalog id (`""` =
/// Windows Default → reset). Returns `false` only for an unknown id so
/// settings reverts.
pub fn apply(id: &str) -> bool {
    let use_variant =
        coredesk_core::taskbar_themes::variant_feature(id).is_some_and(feature_enabled);
    let wire = if id.is_empty() {
        return reset();
    } else {
        coredesk_core::taskbar_themes::theme_wire(id, use_variant)
    };
    let Some(wire) = wire else {
        return false;
    };
    let vertical = taskbar_is_vertical();
    let mut guard = lock();
    let tb = guard.get_or_insert_with(|| Tb {
        style: String::new(),
        wire: String::new(),
        cmd_hwnd: 0,
        hello_sent: false,
        suppressed: false,
    });
    tb.style = id.to_string();
    tb.wire = wire;
    ARMED.store(true, Ordering::Release);
    if let Some((hwnd, _)) = crate::tap::ensure_resident() {
        if vertical {
            // Keep the theme armed but send the reset: the resident restores
            // originals and our property writes stay out of the vertical
            // layout churn until the taskbar goes horizontal again.
            let _ = crate::tap::send_copydata(hwnd, CDS_TB_RESET, &[]);
            tb.suppressed = true;
            tb.cmd_hwnd = 0;
        } else {
            tb.suppressed = false;
            if !deliver(hwnd, tb) {
                tb.cmd_hwnd = 0; // delivery pending — `ensure` retries next tick
            }
        }
    }
    true
}

// ---- shell-explorer crash breaker (NoDiag gate) ------------------------
//
// The armed WUX diagnostics session inside the taskbar-owning explorer pid
// is the verified `0xc0000374` trigger: Microsoft's own
// Windows.UI.Xaml/combase marshal corrupts the shell heap during
// drag-time island churn (reproduced on build 26300.9818 — five clean
// kills across armed instances vs. zero crashes with the session off or
// absent; zero element writes needed). Once `TAP_BREAKER_STRIKES`
// exception-class explorer deaths are observed, the breaker writes the
// `NoDiag` kill-switch build-keyed and restarts the shell — the resident
// comes back inert (no diagnostics, no taskbar styling, stable shell).
// When the OS revision changes the keys mismatch and the resident's
// `diag_disabled()` re-arms on its own; `nodiag_clear_if_stale` sweeps the
// leftover values.
const NODIAG_SUBKEY: &str = r"Software\CoreDeskEngine\Debug";
/// Explorer deaths inside one armed run before the breaker trips.
const TAP_BREAKER_STRIKES: u32 = 2;
/// Process-handle rights for crash attribution only.
const PROC_WATCH_RIGHTS: u32 = 0x1000 | 0x0400; // QUERY_LIMITED_INFORMATION | SYNCHRONIZE
const WAIT_STILL_RUNNING: u32 = 0x102; // WAIT_TIMEOUT
/// A dead-pid handle stays pending this long for a WER-mid-dump corpse.
const PENDING_VERDICT_MS: u128 = 60_000;

/// `true` for exception-class exits (`0xC000****` — heap corruption, AV,
/// fast-fail). A `TerminateProcess(0)` restart and a Task Manager kill
/// exit 0/1 and must not strike.
fn exception_exit(code: u32) -> bool {
    code >= 0xC000_0000
}

struct ShellWatch {
    /// The taskbar-owning pid we believe is alive.
    pid: u32,
    /// Held handle on `pid` — keeps its exit code readable after death.
    /// Stored as usize so the watch stays `Send`.
    proc: usize,
    /// Handles of pids that vanished from `Shell_TrayWnd` but whose process
    /// object has not signaled yet (WER suspends the corpse mid-dump); the
    /// verdict lands on a later tick.
    pending: Vec<(usize, std::time::Instant)>,
    strikes: u32,
}

static SHELL_WATCH: Mutex<Option<ShellWatch>> = Mutex::new(None);

fn open_shell_proc(pid: u32) -> usize {
    unsafe { OpenProcess(PROC_WATCH_RIGHTS, windows_core::BOOL(0), pid) as usize }
}

fn judge_proc(proc: usize) -> Option<bool> {
    // Some(crashed) / Some(clean-exit) / None(still dying).
    let proc = proc as *mut core::ffi::c_void;
    if unsafe { WaitForSingleObject(proc, 0) } == WAIT_STILL_RUNNING {
        return None;
    }
    let mut code = 0u32;
    let ok = unsafe { GetExitCodeProcess(proc, &mut code) }.0 != 0;
    Some(ok && exception_exit(code))
}

fn reg_dword(root: isize, sub: &str, name: &str) -> Option<u32> {
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let sub = widestr(sub);
    let name = widestr(name);
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            core::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (status == 0).then_some(data)
}

/// `<build>.<UBR>` split into numbers — same source the resident's
/// `diag_disabled` reads (`CurrentBuild` + `UBR` under `HKLM`).
fn current_build_ubr() -> (u32, u32) {
    const HKEY_LOCAL_MACHINE: isize = -0x7FFF_FFFEisize; // 0xFFFFFFFF80000002
    const RRF_RT_REG_SZ: u32 = 0x02;
    const CV: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let ubr = reg_dword(HKEY_LOCAL_MACHINE, CV, "UBR").unwrap_or(0);
    let sub = widestr(CV);
    let name = widestr("CurrentBuild");
    let mut buf = [0u16; 64];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    let build = if status == 0 {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len]).parse().unwrap_or(0)
    } else {
        0
    };
    (build, ubr)
}

/// Writes `NoDiag`=1 + `NoDiagBuild`/`NoDiagUbr` under HKCU — the resident
/// reads them on its next diagnostics-install attempt (post-restart
/// re-injection) and stays inert for exactly this OS revision.
fn nodiag_write() -> bool {
    const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize; // 0xFFFFFFFF80000001
    const KEY_WRITE: u32 = 0x2_0006; // KEY_SET_VALUE | KEY_CREATE_SUB_KEY
    const REG_DWORD: u32 = 4;
    let sub = widestr(NODIAG_SUBKEY);
    let mut hkey: isize = 0;
    if unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            0,
            PCWSTR::null(),
            0,
            KEY_WRITE,
            core::ptr::null(),
            &mut hkey,
            core::ptr::null_mut(),
        )
    } != 0
    {
        return false;
    }
    let (build, ubr) = current_build_ubr();
    let set = |name: &str, v: u32| {
        let name = widestr(name);
        unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                0,
                REG_DWORD,
                (&v as *const u32).cast(),
                4,
            ) == 0
        }
    };
    // `NoDiagDll` (REG_SZ) bans exactly this resident image — a rebuilt DLL
    // mismatches it and re-arms diagnostics on the same OS revision.
    let set_sz = |name: &str, v: &str| {
        const REG_SZ: u32 = 1;
        let name = widestr(name);
        let data: Vec<u16> = v.encode_utf16().chain(core::iter::once(0)).collect();
        unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            ) == 0
        }
    };
    let ok = set("NoDiag", 1) && set("NoDiagBuild", build) && set("NoDiagUbr", ubr);
    if let Some(tag) = crate::tap::tap_image_tag() {
        let _ = set_sz("NoDiagDll", &tag);
    }
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    ok
}

/// `true` while the NoDiag ban actually gates THIS build's deployed
/// resident — same contract the resident's `diag_disabled` enforces.
/// Lets the app's crash-loop guard tell "styles already inert, churn is
/// not ours" from "still a live suspect".
pub fn nodiag_active() -> bool {
    const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize;
    if reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiag") != Some(1) {
        return false;
    }
    let b = reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiagBuild");
    let u = reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiagUbr");
    let (Some(b), Some(u)) = (b, u) else {
        return true; // bare kill-switch — everything diagnostic is gated
    };
    if (b, u) != current_build_ubr() {
        return false;
    }
    const RRF_RT_REG_SZ: u32 = 0x02;
    let sub = widestr(NODIAG_SUBKEY);
    let name = widestr("NoDiagDll");
    let mut buf = [0u16; 128];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return true; // no dll key — banned for every resident on this build
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let banned = String::from_utf16_lossy(&buf[..len]);
    crate::tap::tap_image_tag().is_none_or(|tag| tag == banned)
}

/// Removes the build-keyed flag once the OS no longer matches it — a
/// `NoDiag` alone (manual experiment switch, no build keys) is never
/// touched.
fn nodiag_clear_if_stale() {
    const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize;
    if reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiag") != Some(1) {
        return;
    }
    let Some(b) = reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiagBuild") else {
        return;
    };
    let Some(u) = reg_dword(HKEY_CURRENT_USER, NODIAG_SUBKEY, "NoDiagUbr") else {
        return;
    };
    if (b, u) == current_build_ubr() {
        return;
    }
    let sub = widestr(NODIAG_SUBKEY);
    let mut hkey: isize = 0;
    const KEY_WRITE: u32 = 0x2_0006;
    if unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            0,
            PCWSTR::null(),
            0,
            KEY_WRITE,
            core::ptr::null(),
            &mut hkey,
            core::ptr::null_mut(),
        )
    } == 0
    {
        for n in ["NoDiag", "NoDiagBuild", "NoDiagUbr", "NoDiagDll"] {
            let n = widestr(n);
            unsafe {
                let _ = RegDeleteValueW(hkey, PCWSTR(n.as_ptr()));
            }
        }
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        tracing::info!("taskbar styler: stale NoDiag flag cleared — OS revision changed");
    }
}

/// Called from `ensure` while armed — attributes shell-explorer deaths to
/// the armed diagnostics session and trips the NoDiag breaker on repeats.
fn shell_crash_watch(shell_pid: u32) {
    let mut trip = false;
    {
        let mut guard = SHELL_WATCH.lock().unwrap_or_else(|p| p.into_inner());
        let w = guard.get_or_insert_with(|| ShellWatch {
            pid: shell_pid,
            proc: open_shell_proc(shell_pid),
            pending: Vec::new(),
            strikes: 0,
        });
        let now = std::time::Instant::now();
        // Verdicts for pids that left Shell_TrayWnd on earlier ticks.
        w.pending.retain_mut(|(proc, at)| match judge_proc(*proc) {
            None if now.duration_since(*at).as_millis() < PENDING_VERDICT_MS => true,
            verdict => {
                if verdict == Some(true) {
                    w.strikes += 1;
                    tracing::warn!(strikes = w.strikes, "taskbar styler: shell explorer died by exception — suspected diagnostics crash");
                }
                unsafe {
                    let _ = CloseHandle(*proc as *mut core::ffi::c_void);
                }
                false
            }
        });
        if w.pid != shell_pid {
            // Shell pid changed — the old one is dead or dying; judge it on
            // this and later ticks via `pending`.
            if w.proc != 0 {
                w.pending.push((w.proc, now));
            }
            w.pid = shell_pid;
            w.proc = open_shell_proc(shell_pid);
        }
        if w.strikes >= TAP_BREAKER_STRIKES {
            w.strikes = 0;
            trip = true;
        }
    }
    if trip && nodiag_write() {
        let (b, u) = current_build_ubr();
        tracing::warn!(
            build = b,
            ubr = u,
            strikes = TAP_BREAKER_STRIKES,
            "taskbar styler: NoDiag breaker tripped — \
             diagnostics stay off on this OS revision; restarting explorer"
        );
        std::thread::spawn(crate::tap::restart_explorer);
    }
}

/// `Windows Default` — tell the resident styler to restore originals and
/// stop keeping the DLL alive on our account. Does **not** inject: a dead
/// resident has nothing to reset.
pub fn reset() -> bool {
    let mut guard = lock();
    if let Some(hwnd) = crate::tap::resident_window() {
        let _ = crate::tap::send_copydata(hwnd, CDS_TB_RESET, &[]);
    }
    if let Some(tb) = guard.as_mut() {
        tb.style.clear();
        tb.wire.clear();
    }
    ARMED.store(false, Ordering::Release);
    true
}

/// Periodic liveness driven by the app's anchor tick: re-injects and
/// re-delivers the wire when explorer restarted under an armed style.
/// Otherwise sends a `HELLO` — cheap poke that re-arms the DLL's watcher
/// retry and the death watch if either was lost.
pub fn ensure() {
    // A build-keyed flag from an older OS revision is swept even while
    // disarmed — the resident re-arms diagnostics on its next install.
    nodiag_clear_if_stale();
    if !is_armed() {
        return;
    }
    let Some((hwnd, _)) = crate::tap::ensure_resident() else {
        return;
    };
    // Attribute shell-explorer deaths to the armed session; on repeats the
    // breaker writes `NoDiag` and restarts the shell inert.
    if let Some((_, shell_pid)) = crate::tap::find_taskbar() {
        shell_crash_watch(shell_pid);
    }
    let vertical = taskbar_is_vertical();
    let mut guard = lock();
    let Some(tb) = guard.as_mut() else {
        return;
    };
    if vertical {
        // Once per transition — the reset is idempotent but each send runs
        // a full restore sweep inside the host.
        if !tb.suppressed {
            let _ = crate::tap::send_copydata(hwnd, CDS_TB_RESET, &[]);
            tb.suppressed = true;
            tb.cmd_hwnd = 0;
        }
        return;
    }
    if tb.suppressed {
        tb.suppressed = false;
        tb.cmd_hwnd = 0; // force re-delivery of the armed theme
    }
    // Resident identity decides re-delivery — a same-tick reinjection may
    // have already consumed the `fresh` flag. A `Build`-less window is a
    // dormant same-build resident — re-deliver so its revive hello fires.
    if tb.cmd_hwnd != hwnd.0 as usize || crate::tap::resident_version(hwnd) == 0 {
        let _ = deliver(hwnd, tb);
    }
}

/// App shutdown — restore stock styling; the transparency teardown in
/// `tap::shutdown` unloads the resident itself.
pub fn shutdown() {
    let _ = reset();
}
