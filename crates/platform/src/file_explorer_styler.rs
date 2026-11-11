//! Host-side controller for the File Explorer material — drives the real
//! [ExplorerBlurMica](https://github.com/Maplespe/ExplorerBlurMica)
//! extension binary (vendored `third_party/ebm/ExplorerBlurMica.dll`,
//! LGPLv3). Upstream's `*.cpp` sources are private, so the release binary
//! is the only bit-exact engine; this module loads THE DLL ITSELF rather
//! than re-implementing its pipeline.
//!
//! Delivery — `EBML`/`EBMU` COPYDATA through the `coredesk_tap`
//! resident already injected in each explorer process (the retired
//! `CDES-FES1` layout/XAML wire is gone; `FELR` remains as a
//! reset-only broadcast for residents still carrying it):
//!
//! ```text
//! apply_effect → deploy DLL + write config.ini → per cabinet-owning
//!   explorer pid: ensure_resident_at → EBML <utf16 dll path>
//!     → resident LoadLibraryW(path) → EBM's DllMain self-installs
//! reset       → EBMU broadcast to every resident window
//!     → resident FreeLibrary → DllMain detach restores originals
//! ```
//!
//! Why not COM registration: `Drive\shellex\FolderExtensions` entries
//! are only honoured under `HKLM` (admin) — upstream's `register.cmd`
//! elevates for exactly that reason. A `LoadLibraryW` lands the same
//! `DllMain` in the same process with zero admin and covers
//! separate-process explorers wherever a resident can be injected.
//!
//! `config.ini` sits beside the deployed DLL — EBM reads it from its
//! own module directory (`[config]` effect/flags, `[light]`/`[dark]`
//! RGBA; it handles the system theme switch itself).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows_core::PCWSTR;

windows_core::link!("user32.dll" "system" fn GetClassNameW(hwnd: crate::bindings::HWND, lpclassname: windows_core::PWSTR, nmaxcount: i32) -> i32);
windows_core::link!("user32.dll" "system" fn GetForegroundWindow() -> crate::bindings::HWND);
windows_core::link!("advapi32.dll" "system" fn RegOpenKeyExW(hkey: isize, subkey: PCWSTR, options: u32, sam: u32, result: *mut isize) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegQueryValueExW(hkey: isize, valuename: PCWSTR, reserved: *const u32, r#type: *mut u32, data: *mut u8, cbdata: *mut u32) -> i32);
windows_core::link!("advapi32.dll" "system" fn RegCloseKey(hkey: isize) -> i32);
windows_core::link!("kernel32.dll" "system" fn OpenProcess(dwdesiredaccess: u32, binherithandle: windows_core::BOOL, dwprocessid: u32) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject: *mut core::ffi::c_void) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GetExitCodeProcess(hprocess: *mut core::ffi::c_void, lpexitcode: *mut u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(hhandle: *mut core::ffi::c_void, dwmilliseconds: u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn TerminateProcess(hprocess: *mut core::ffi::c_void, uexitcode: u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn QueryFullProcessImageNameW(hprocess: *mut core::ffi::c_void, dwflags: u32, lpexename: windows_core::PWSTR, lpdwsize: *mut u32) -> windows_core::BOOL);

/// `PROCESS_QUERY_LIMITED_INFORMATION` — enough to test the pid's death.
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
/// `PROCESS_TERMINATE` — kill a still-armed pid once the breaker trips.
const PROCESS_TERMINATE: u32 = 0x0001;
/// `SYNCHRONIZE` — lets `WaitForSingleObject` observe the process's death.
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
/// `WAIT_TIMEOUT` — the handle is not signaled: still running (or a
/// WER-suspended corpse mid-dump).
const WAIT_STILL_RUNNING: u32 = 0x102;
/// `STILL_ACTIVE` — reported while WER holds the process open.
const EXIT_STILL_ACTIVE: u32 = 259;

/// `COPYDATASTRUCT.dwData` values — must match `crates/tap/src/command.rs`.
/// `EBML` = load `ExplorerBlurMica.dll` (payload: UTF-16 path + NUL);
/// `EBMU` = unload it. `FESR` = the legacy `fe_glass` reset, kept so a
/// still-armed resident from an older build disarms its detours before
/// EBM takes over (one broadcast per apply — not periodic).
/// `FELR` = reset the retired `CDES-FES1` mux wire — broadcast-only, so a
/// resident carrying a layout/clear from an older build restores stock
/// chrome. The wire itself (`FELT`) is gone: layout presets were removed.
const CDS_EBM_LOAD: usize = u32::from_be_bytes(*b"EBML") as usize;
const CDS_EBM_UNLOAD: usize = u32::from_be_bytes(*b"EBMU") as usize;
const CDS_FE_RESET: usize = u32::from_be_bytes(*b"FESR") as usize;
const CDS_FE_LAYOUT_RESET: usize = u32::from_be_bytes(*b"FELR") as usize;

/// UI-facing EBM configuration — mirrors `config.ini`'s `[config]` flags
/// plus the `[light]`/`[dark]` RGBA strings (`#AARRGGBB`).
pub struct FeGlassConfig<'a> {
    /// `""` = off; `"blur"`, `"acrylic"`, `"mica"`, `"blurclear"`,
    /// `"micaalt"` — EBM `effect=` 0..4.
    pub effect: &'a str,
    pub clear_address: bool,
    pub clear_bar_bg: bool,
    pub clear_winui_bg: bool,
    pub show_line: bool,
    pub light_rgba: &'a str,
    pub dark_rgba: &'a str,
}

/// Owned snapshot of the last applied config — `ensure()` re-delivers it
/// to newly-appearing explorer pids; `epoch` bumps on every apply so a
/// config change reloads EBM in already-covered pids (it reads
/// `config.ini` once per `LoadLibrary`).
#[derive(Default)]
struct Fe {
    effect: String,
    clear_address: bool,
    clear_bar_bg: bool,
    clear_winui_bg: bool,
    show_line: bool,
    light_rgba: String,
    dark_rgba: String,
    /// Ini-generation counter — bumps ONLY when a `config.ini`-relevant
    /// field changes (effect/flags/rgba). A trailing pid gets `EBMU`+`EBML`
    /// (fresh config read); layout-only and identical applies bump nothing,
    /// so routine delivery never unloads the live module.
    epoch: u64,
    /// pid → (cmd window, epoch delivered). `ensure()` only wires pids
    /// that are new, stale-epoch, or whose resident restarted.
    delivered: std::collections::HashMap<u32, (usize, u64)>,
    /// The deployed `ExplorerBlurMica.dll` path `EBML` hands to residents
    /// (hash-versioned — an engine upgrade reloads through a fresh path).
    dll_path: String,
    /// Last `deliver_all` fan-out signature — the debug line logs only when
    /// pids/readiness/epochs change, not every 2 s `ensure()` tick.
    delivery_sig: String,
    /// pid → when the cabinet owner was first observed. EBM hooks whatever
    /// XAML the pid hosts — on the shared shell explorer that includes the
    /// taskbar and the Alt-Tab switcher — so `EBML` waits until the pid
    /// survived a whole settle window instead of landing mid-creation.
    first_seen: HashMap<u32, Instant>,
    /// pid → when `EBML` was last delivered + a process handle held open
    /// from that moment — a pid that dies inside `EBM_CRASH_WINDOW` of a
    /// delivery is a suspected EBM crash, and the held handle keeps its
    /// EXIT CODE readable afterwards: `GetExitCodeProcess` returns the
    /// exception code on a crash but 0 on a natural exit (closing the last
    /// cabinet exits a separate-process explorer) — that's the
    /// crash-vs-shutdown discriminator `OpenProcess`-polling can't see.
    delivered_at: HashMap<u32, Delivery>,
    /// When the last ini-relevant change landed. A burst of applies (the
    /// settings tint slider pushes one per tick) must collapse into ONE
    /// `EBMU`+`EBML` reload — `FreeLibrary` mid-detour is the crash-prone
    /// path, so deliveries wait `EBM_RELOAD_SETTLE` for writes to settle.
    last_ini_change: Option<Instant>,
    /// Suspected EBM crashes this session; `EBM_MAX_STRIKES` trips `halted`.
    strikes: u32,
    /// Circuit breaker: no more `EBML` this session (unloads still flow).
    halted: bool,
    /// `CabinetWClass` hwnds the safe DWM fallback touched — restored to
    /// `DWMSBT_AUTO` when EBM re-arms or the material resets. Pruned to
    /// live cabinet hwnds on every fallback pass.
    fallback_hwnds: std::collections::HashSet<usize>,
    /// The backdrop value the fallback last wrote — a banned effect
    /// change (mica→micaalt) reapplies every live hwnd.
    fallback_kind: i32,
}

static FE: Mutex<Option<Fe>> = Mutex::new(None);

/// One `delivered_at` entry — the delivery instant plus a `SYNCHRONIZE`
/// handle opened at deliver time. `Drop` closes it; `delivered` re-entries
/// and map clears therefore never leak handles.
struct Delivery {
    at: Instant,
    proc: *mut core::ffi::c_void,
}
impl Drop for Delivery {
    fn drop(&mut self) {
        if !self.proc.is_null() {
            unsafe {
                let _ = CloseHandle(self.proc);
            }
        }
    }
}
unsafe impl Send for Delivery {}

/// Whether a File Explorer effect is armed — read lock-free.
static ARMED: AtomicBool = AtomicBool::new(false);

/// `true` while a File Explorer effect is armed.
pub fn is_armed() -> bool {
    ARMED.load(Ordering::Acquire)
}

fn lock() -> std::sync::MutexGuard<'static, Option<Fe>> {
    FE.lock().unwrap_or_else(|p| p.into_inner())
}

fn widestr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// EBM `effect` name → `config.ini` integer.
fn effect_id(effect: &str) -> Option<u32> {
    match effect {
        "blur" => Some(0),
        "acrylic" => Some(1),
        "mica" => Some(2),
        "blurclear" => Some(3),
        "micaalt" => Some(4),
        _ => None,
    }
}

/// `%LOCALAPPDATA%\CoreDesk Engine\residents` — same home the tap resident
/// uses; stable for the resident-side `LoadLibraryW` to read.
fn residents_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("CoreDeskEngine")
        .join("residents");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// The vendored engine compiled into the binary — the canonical source for
/// deployment: a next-to-exe file is not part of the contract (installers,
/// `--portable` copies and dev layouts stage it differently). The 658 KB
/// `include_bytes!` is cheap insurance: on a machine where the sidecar file
/// was never staged, the material still deploys.
static EBM_DLL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../third_party/ebm/ExplorerBlurMica.dll"
));

/// Source bytes to deploy. `COREDESK_EBM_DLL=<path>` overrides for engine
/// development only — anything loaded into explorer.exe must come from an
/// explicit opt-in or the bytes this build was tested with.
fn ebm_source_bytes() -> Option<Vec<u8>> {
    match std::env::var("COREDESK_EBM_DLL") {
        Ok(p) => match std::fs::read(&p) {
            Ok(b) => Some(b),
            Err(e) => {
                // A set-but-unreadable override must not silently fall back —
                // the developer asked for specific bytes.
                tracing::warn!(path = %p, error = %e, "COREDESK_EBM_DLL unreadable; using vendored engine");
                Some(EBM_DLL.to_vec())
            }
        },
        Err(_) => Some(EBM_DLL.to_vec()),
    }
}

/// FNV-1a over the engine bytes — labels the deploy dir; every content change
/// gets a new path (never an overwrite of a DLL a live explorer has mapped).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `residents/ebm-<hash>` — the deployed engine's versioned home. A new build
/// lands in a NEW path: a running explorer's mapped DLL is never under the
/// write target, and `EBMU`+`EBML` swaps engines live in the same pid.
fn ebm_deploy_dir(bytes: &[u8]) -> Option<PathBuf> {
    Some(residents_dir()?.join(format!("ebm-{:016x}", fnv1a64(bytes))))
}

/// Deploys the vendored DLL into its hash-versioned dir. `Some(dst)` is only
/// returned when the file on disk is byte-identical to the source — a
/// byte-equal destination is accepted without rewriting; a mismatch is
/// rewritten (new path ⇒ no live-share lock to fight), and any write/verify
/// failure is a hard error: LoadLibrary-ing stale bytes would arm an
/// unknown engine.
fn deploy_dll() -> Option<PathBuf> {
    let src = ebm_source_bytes()?;
    let dir = ebm_deploy_dir(&src)?;
    let dst = dir.join("ExplorerBlurMica.dll");
    match std::fs::read(&dst) {
        Ok(d) if d == src => {
            tracing::debug!(
                dst = %dst.display(),
                "deploy_dll: destination already matches the vendored DLL — copy skipped"
            );
            Some(dst)
        }
        _ => {
            if std::fs::create_dir_all(&dir).is_err() || std::fs::write(&dst, &src).is_err() {
                tracing::warn!(
                    dst = %dst.display(),
                    "ExplorerBlurMica.dll deploy failed — destination cannot be written"
                );
                return None;
            }
            match std::fs::read(&dst) {
                Ok(d) if d == src => {
                    tracing::debug!(
                        dst = %dst.display(),
                        bytes = src.len(),
                        "deploy_dll: destination written"
                    );
                    Some(dst)
                }
                _ => {
                    tracing::warn!(
                        dst = %dst.display(),
                        "ExplorerBlurMica.dll deploy failed — written bytes failed verification"
                    );
                    None
                }
            }
        }
    }
}

/// `#AARRGGBB` (or `#RRGGBB`) → `(r, g, b, a)` for `config.ini`.
fn parse_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    let v = match hex.len() {
        8 => u32::from_str_radix(hex, 16).ok()?,
        6 => u32::from_str_radix(hex, 16).ok()? | 0xFF00_0000,
        _ => return None,
    };
    Some((
        ((v >> 16) & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        (v & 0xFF) as u8,
        (v >> 24) as u8,
    ))
}

/// Builds the `config.ini` contents for `cfg` — byte-format identical
/// to upstream's `[config]` + `[light]` + `[dark]` layout.
fn config_ini_contents(cfg: &FeGlassConfig) -> String {
    let l = parse_rgba(cfg.light_rgba).unwrap_or((220, 220, 220, 160));
    let d = parse_rgba(cfg.dark_rgba).unwrap_or((0, 0, 0, 120));
    let b = |v: bool| if v { "true" } else { "false" };
    format!(
        "[config]\r\neffect={}\r\nclearAddress={}\r\nclearBarBg={}\r\nclearWinUIBg={}\r\nshowLine={}\r\n[light]\r\nr={}\r\ng={}\r\nb={}\r\na={}\r\n[dark]\r\nr={}\r\ng={}\r\nb={}\r\na={}\r\n",
        effect_id(cfg.effect).unwrap_or(1),
        b(cfg.clear_address),
        b(cfg.clear_bar_bg),
        b(cfg.clear_winui_bg),
        b(cfg.show_line),
        l.0,
        l.1,
        l.2,
        l.3,
        d.0,
        d.1,
        d.2,
        d.3,
    )
}

/// Writes `config.ini` beside the deployed DLL. Identical existing bytes
/// are skipped (EBM may hold the file open read-only — a skipped write
/// can never hit a sharing violation); a real write failure surfaces the
/// io error to the caller with the path attached.
fn write_config_ini(dir: &Path, cfg: &FeGlassConfig) -> std::io::Result<()> {
    let path = dir.join("config.ini");
    let ini = config_ini_contents(cfg);
    match std::fs::read(&path) {
        Ok(existing) if existing == ini.as_bytes() => {
            tracing::debug!(
                ini = %path.display(),
                "config.ini already matches the requested config — write skipped"
            );
            Ok(())
        }
        Ok(_) | Err(_) => {
            let bytes = ini.len();
            std::fs::write(&path, ini).inspect(|_| {
                tracing::debug!(ini = %path.display(), bytes, "config.ini written");
            })
        }
    }
}

// ------------------------------------------------------------- cabinets ----

/// Every `CabinetWClass` that is a real File Explorer window — frames
/// with a `WorkerW` child are the legacy Control Panel shell, which has
/// no XAML islands and isn't targeted.
fn cabinet_windows() -> Vec<crate::bindings::HWND> {
    struct Ctx {
        out: Vec<crate::bindings::HWND>,
    }
    unsafe extern "system" fn cb(
        hwnd: crate::bindings::HWND,
        lparam: crate::bindings::LPARAM,
    ) -> windows_core::BOOL {
        let _ = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
            let mut cls = [0u16; 64];
            let n = unsafe { GetClassNameW(hwnd, windows_core::PWSTR(cls.as_mut_ptr()), 64) };
            if n <= 0 || String::from_utf16_lossy(&cls[..n as usize]) != "CabinetWClass" {
                return;
            }
            let worker = widestr("WorkerW");
            let legacy = unsafe {
                crate::bindings::FindWindowExW(
                    Some(hwnd),
                    None,
                    PCWSTR(worker.as_ptr()),
                    PCWSTR::null(),
                )
            };
            if !legacy.0.is_null() {
                return;
            }
            ctx.out.push(hwnd);
        }));
        windows_core::BOOL(1)
    }
    let mut ctx = Ctx { out: Vec::new() };
    unsafe {
        let _ = crate::bindings::EnumWindows(
            Some(cb),
            crate::bindings::LPARAM(&mut ctx as *mut Ctx as isize),
        );
    }
    ctx.out
}

/// One anchor `CabinetWClass` per owning explorer pid — inject targets
/// for `ensure_resident_at` (separate-process explorers need their own
/// resident; the shell explorer's resident can't reach them).
fn cabinet_anchors() -> Vec<(u32, crate::bindings::HWND)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for hwnd in cabinet_windows() {
        let mut pid = 0u32;
        unsafe {
            let _ = crate::bindings::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != 0 && seen.insert(pid) {
            out.push((pid, hwnd));
        }
    }
    out
}

// ------------------------------------------------------------- delivery ----

/// A cabinet pid must be observed this long before `EBML` — one `ensure()`
/// tick spans it — so EBM's `DllMain` hooks never land in a pid whose XAML
/// is still constructing. Just-injected cabinet processes are exactly where
/// the Switcher/taskbar APPCRASHes were reported.
const EBM_PID_WARMUP: Duration = Duration::from_secs(3);
/// A delivered pid dying within this window counts as a suspected EBM
/// crash. EBM's heap corruption detonates on USER interaction (window
/// drag, snap) which lags delivery — 90 s missed exactly that path; 5 min
/// still bounds the attribution (a healthy shell does not die that often).
const EBM_CRASH_WINDOW: Duration = Duration::from_secs(300);
/// Suspected crashes before `EBML` stops for the session.
const EBM_MAX_STRIKES: u32 = 2;
/// How long `deliver_all` waits after the last ini change before issuing
/// an `EBMU`+`EBML` reload — collapses slider bursts into one reload.
const EBM_RELOAD_SETTLE: Duration = Duration::from_millis(400);
/// A persisted breaker older than this expires — a long grace so a Windows
/// build that kills this engine is not retried daily; the `build`/`dll`
/// keys still retry instantly when either side changes.
const BREAKER_TTL_SECS: u64 = 30 * 24 * 60 * 60;

/// `residents/ebm-breaker.json` — `{"effect","build","dll","at"}`.
/// Persists the crash breaker across app restarts: without it every launch
/// re-arms EBM and the crash loop resumes exactly as before.
fn breaker_path() -> Option<PathBuf> {
    Some(residents_dir()?.join("ebm-breaker.json"))
}

/// The persisted ban record. `build` is `<build>.<UBR>` (EBM's detours
/// broke on a UBR bump — same base build, new offsets); `dll` is the
/// fnv tag of the deployed engine bytes so a shipped fix un-bans itself.
struct BreakerRecord {
    effect: String,
    build: String,
    dll: String,
    at: u64,
}

/// Reads the persisted breaker record — missing `build`/`dll` keys (an
/// older format's record) parse as empty, and empty keys MATCH anything:
/// a pre-upgrade ban must keep its pid-killing power on the first run.
fn breaker_record() -> Option<BreakerRecord> {
    let text = std::fs::read_to_string(breaker_path()?).ok()?;
    // Tiny hand-parse — the values are fixed ASCII ids/digits, so no
    // escapes can appear.
    let find = |key: &str| -> Option<String> {
        let pat = format!("\"{key}\":");
        let rest = text.split(&pat).nth(1)?.trim_start();
        if rest.starts_with('"') {
            Some(rest.trim_start_matches('"').split('"').next()?.to_owned())
        } else {
            Some(
                rest.trim_end_matches(['}', ' ', '\n', '\r', '\t', ','])
                    .to_owned(),
            )
        }
    };
    let effect = find("effect").unwrap_or_default();
    let at = find("at")?.parse().ok()?;
    Some(BreakerRecord {
        effect,
        build: find("build").unwrap_or_default(),
        dll: find("dll").unwrap_or_default(),
        at,
    })
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `<build>.<UBR>` — UBR distinguishes monthly revisions inside one build
/// number; the 26100.9711→26100.9818 bump is exactly where the vendored
/// engine's offsets went stale.
fn os_build_tag() -> String {
    let mut tag = crate::desktop::os_build().to_string();
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const KEY_READ: u32 = 0x2_0019;
    let sub = widestr("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let name = widestr("UBR");
    let mut hkey: isize = 0;
    if unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(sub.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
    } == 0
    {
        let mut ubr: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let _ = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                core::ptr::null(),
                core::ptr::null_mut(),
                &mut ubr as *mut u32 as *mut u8,
                &mut size,
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        if ubr != 0 {
            tag = format!("{tag}.{ubr}");
        }
    }
    tag
}

/// `true` while the persisted record names THIS Windows revision AND THIS
/// engine binary. The ban is keyed to `(build, dll)`, not the effect —
/// EBM's `DllMain` detours install whatever material the ini asks for, so
/// a build that kills `mica` will kill `blur` too. A Windows update or a
/// shipped engine fix flips one key and the contract retries fresh.
/// `true` while the persisted record covers THIS Windows revision AND the
/// currently deployed engine binary — i.e. the File Explorer effect
/// surface is inert and explorer churn cannot be attributed to it.
pub fn ebm_banned() -> bool {
    breaker_banned(&ebm_dll_tag())
}

fn breaker_banned(dll_tag: &str) -> bool {
    match breaker_record() {
        Some(r) => {
            let banned = (r.build.is_empty() || r.build == os_build_tag())
                && (r.dll.is_empty() || r.dll == dll_tag)
                && unix_now().saturating_sub(r.at) < BREAKER_TTL_SECS;
            if banned {
                tracing::debug!(
                    crashed_effect = %r.effect,
                    build = %r.build,
                    "EBM breaker record matched — engine stays banned"
                );
            }
            banned
        }
        None => false,
    }
}

/// A record exists but does NOT cover this `(build, dll)` — a Windows
/// update or engine swap makes it stale; clearing it is what "the app
/// heals itself when the world changes" looks like on disk.
fn breaker_drop_if_stale(dll_tag: &str) {
    if let Some(r) = breaker_record() {
        if r.build != os_build_tag() || r.dll != dll_tag {
            breaker_clear();
        }
    }
}

fn breaker_save(effect: &str, dll_tag: &str) {
    let Some(path) = breaker_path() else { return };
    let _ = std::fs::write(
        path,
        format!(
            "{{\"effect\":\"{effect}\",\"build\":\"{}\",\"dll\":\"{dll_tag}\",\"at\":{}}}",
            os_build_tag(),
            unix_now()
        ),
    );
}

fn breaker_clear() {
    if let Some(path) = breaker_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// `EBMU` broadcast — unloads the material in every resident regardless of
/// delivery bookkeeping. Used by `reset_ebm`, by a halted apply (a pid can
/// still carry the module), and by the crash breaker when it trips.
fn broadcast_ebm_unload() {
    for hwnd in crate::tap::resident_windows_all() {
        let _ = crate::tap::send_copydata(hwnd, CDS_EBM_UNLOAD, &[]);
    }
}

/// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + image-name check —
/// `true` only when the pid is alive AND still `explorer.exe`. `delivered`
/// keeps entries for dead pids until the next clear, and a recycled pid
/// must never be terminated as an EBM victim.
fn pid_is_explorer(pid: u32) -> bool {
    let h = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            windows_core::BOOL(0),
            pid,
        )
    };
    if h.is_null() {
        return false;
    }
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(h, 0, windows_core::PWSTR(buf.as_mut_ptr()), &mut len)
    };
    unsafe {
        let _ = CloseHandle(h);
    }
    if ok.0 == 0 {
        return false;
    }
    let name = String::from_utf16_lossy(&buf[..len as usize]);
    name.rsplit('\\')
        .next()
        .is_some_and(|f| f.eq_ignore_ascii_case("explorer.exe"))
}

/// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` — a live pid answers.
fn pid_alive(pid: u32) -> bool {
    let h = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            windows_core::BOOL(0),
            pid,
        )
    };
    if h.is_null() {
        return false;
    }
    unsafe {
        let _ = CloseHandle(h);
    }
    true
}

/// A task-switcher UI is foreground. On the shared shell explorer, EBM's
/// `DllMain` installs hooks process-wide — delivering while `Switcher`/
/// `MultitaskingView` XAML is up is the reported APPCRASH path, so the
/// whole batch defers one tick instead of sending mid-gesture.
fn task_switcher_foreground() -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return false;
    }
    let mut cls = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, windows_core::PWSTR(cls.as_mut_ptr()), 64) };
    if n <= 0 {
        return false;
    }
    matches!(
        &*String::from_utf16_lossy(&cls[..n as usize]),
        "MultitaskingViewFrame" | "TaskSwitcherWnd" | "ForegroundStaging"
    )
}

/// Sends `EBMU`+`EBML` to one pid's resident. Unload-first makes a
/// config change re-read `config.ini` in already-loaded pids; it is a
/// no-op where EBM was never loaded.
fn deliver_ebm(cmd: crate::bindings::HWND, dll_path_utf16: &[u8]) {
    crate::tap::revive_if_dormant(cmd, "CoreDeskTAP");
    let _ = crate::tap::send_copydata(cmd, CDS_EBM_UNLOAD, &[]);
    let _ = crate::tap::send_copydata(cmd, CDS_EBM_LOAD, dll_path_utf16);
}

/// Fans `EBML` out to every cabinet-owning explorer pid — **only** to
/// pids that are new, whose resident restarted (cmd hwnd changed), or
/// whose ini-generation trails. This is the anti-storm rule: `ensure()`
/// calls us every anchor tick but wires only fire on change.
fn deliver_all() {
    let wide: Vec<u8> = lock()
        .as_ref()
        .map(|fe| fe.dll_path.clone())
        .filter(|p| !p.is_empty() && Path::new(p).is_file())
        .map(|dll| {
            dll.encode_utf16()
                .chain(core::iter::once(0))
                .flat_map(|c| c.to_le_bytes())
                .collect()
        })
        .unwrap_or_default();
    let anchors = cabinet_anchors();
    // State-change signature: same pids + same readiness + same epoch means
    // this tick delivers nothing new — the debug line repeats otherwise.
    {
        let mut pids: Vec<u32> = anchors.iter().map(|(pid, _)| *pid).collect();
        pids.sort_unstable();
        let sig = {
            let mut guard = lock();
            let Some(fe) = guard.as_mut() else {
                return;
            };
            format!(
                "{}|{}|{}|{:?}",
                fe.epoch,
                !wide.is_empty(),
                anchors.len(),
                pids
            )
        };
        let mut guard = lock();
        if let Some(fe) = guard.as_mut()
            && fe.delivery_sig != sig
        {
            fe.delivery_sig = sig;
            drop(guard);
            tracing::debug!(
                anchors = anchors.len(),
                dll_ready = !wide.is_empty(),
                "deliver_all: fanning out to cabinet-owning explorer pids"
            );
        }
    }
    // A delivered pid dying mid-flight is the crash signal: the post-loop
    // accounting counts victims of the last batch before another goes out.
    // `EBML` halts for the session after enough strikes — EBMU/reset and
    // resident management stay live.
    let halted = lock().as_ref().is_some_and(|fe| fe.halted);
    let mut live_pids = Vec::new();
    for (pid, anchor) in anchors {
        live_pids.push(pid);
        let Some(cmd) = crate::tap::ensure_resident_at(anchor) else {
            tracing::debug!(pid, "deliver_all: no resident for cabinet pid — skipped");
            continue;
        };
        let mut guard = lock();
        let Some(fe) = guard.as_mut() else {
            return;
        };
        if !halted && !fe.effect.is_empty() && !wide.is_empty() {
            let current = (cmd.0 as usize, fe.epoch);
            if fe.delivered.get(&pid) != Some(&current) {
                let now = Instant::now();
                let first = *fe.first_seen.entry(pid).or_insert(now);
                if now.duration_since(first) < EBM_PID_WARMUP {
                    // Seen for the first time: let the pid's XAML settle; the
                    // next ensure() tick delivers.
                    continue;
                }
                if fe
                    .last_ini_change
                    .is_some_and(|t| now.duration_since(t) < EBM_RELOAD_SETTLE)
                {
                    // Config is still settling (a burst of applies): one
                    // reload with the final ini beats a FreeLibrary storm.
                    continue;
                }
                if task_switcher_foreground() {
                    tracing::debug!(pid, "deliver_all: task switcher foreground — deferred");
                    continue;
                }
                deliver_ebm(cmd, &wide);
                tracing::debug!(pid, epoch = fe.epoch, "deliver_all: EBML sent");
                fe.delivered.insert(pid, current);
                // Hold the pid open so its exit code stays readable after
                // death — `insert` drops (and closes) any prior handle.
                let proc = unsafe {
                    OpenProcess(
                        PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                        windows_core::BOOL(0),
                        pid,
                    )
                };
                fe.delivered_at.insert(pid, Delivery { at: now, proc });
            }
        }
    }
    // Post-delivery crash accounting: a delivered pid gone from the cabinet
    // set that also failed `OpenProcess` inside the window is a strike. A
    // pid that merely closed its last cabinet (still alive — the shared
    // shell explorer) is not.
    let mut trip_halt = false;
    let mut armed_victims = Vec::new();
    {
        let mut guard = lock();
        if let Some(fe) = guard.as_mut() {
            let now = Instant::now();
            let mut dead = Vec::new();
            fe.delivered_at.retain(|pid, d| {
                if live_pids.contains(pid) {
                    return true;
                }
                if now.duration_since(d.at) >= EBM_CRASH_WINDOW {
                    return false; // outside the crash window — forget
                }
                if d.proc.is_null() {
                    // No handle to judge by — fall back to the old probe.
                    if pid_alive(*pid) {
                        return true;
                    }
                    dead.push(*pid);
                    return false;
                }
                if unsafe { WaitForSingleObject(d.proc, 0) } == WAIT_STILL_RUNNING {
                    // Alive, or a WER-suspended corpse mid-dump — judged
                    // next tick once the object signals.
                    return true;
                }
                // Signaled = terminated; the exit code says how. A natural
                // exit (last cabinet closed → `ExitProcess(0)`) is not a
                // strike; an exception code is.
                let mut code = 0u32;
                if unsafe { GetExitCodeProcess(d.proc, &mut code) }.0 != 0
                    && code != 0
                    && code != EXIT_STILL_ACTIVE
                {
                    dead.push(*pid);
                }
                false
            });
            for pid in dead {
                fe.strikes += 1;
                tracing::warn!(
                    pid,
                    strikes = fe.strikes,
                    "explorer pid died shortly after EBML — suspected EBM crash"
                );
            }
            if !fe.halted && fe.strikes >= EBM_MAX_STRIKES {
                fe.halted = true;
                armed_victims = fe
                    .delivered
                    .keys()
                    .chain(fe.delivered_at.keys())
                    .copied()
                    .collect();
                fe.delivered.clear();
                trip_halt = true;
            }
            fe.first_seen.retain(|pid, _| live_pids.contains(pid));
        }
    }
    if trip_halt {
        let effect = lock()
            .as_ref()
            .map(|fe| fe.effect.clone())
            .unwrap_or_default();
        breaker_save(&effect, &ebm_dll_tag());
        tracing::warn!(
            effect = %effect,
            "EBM halted after repeated explorer crashes — material disarmed until the effect changes"
        );
        // Halting must not just stop NEW deliveries: pids that already
        // carry the module keep crashing until it unloads — EBMU restores
        // their originals.
        broadcast_ebm_unload();
        // EBMU is only best-effort: the module self-pins inside a live pid
        // (observed: image still mapped a minute after the unload, and the
        // pid crashed 13 s later) — and seeded heap corruption does not
        // need the hooks active to detonate later. The only certain fix
        // for a still-armed pid is to terminate it; the shell respawns its
        // explorer and a separate-process cabinet just closes — the same
        // blink the crash was about to cause anyway.
        armed_victims.sort_unstable();
        armed_victims.dedup();
        for pid in armed_victims {
            if !pid_is_explorer(pid) {
                continue;
            }
            let h = unsafe { OpenProcess(PROCESS_TERMINATE, windows_core::BOOL(0), pid) };
            if h.is_null() {
                continue;
            }
            unsafe {
                let _ = TerminateProcess(h, 0);
                let _ = CloseHandle(h);
            }
            tracing::warn!(
                pid,
                "terminated still-armed explorer pid after breaker trip"
            );
        }
    }
    // Self-heal: while the breaker holds EBM off, the pure-DWM fallback
    // still delivers a material — `mica`/`micaalt`/`acrylic` are one
    // attribute each and need no injected code. Runs every tick while
    // halted so freshly-opened cabinets are covered; restores stock when
    // the engine re-arms.
    let halted_effect = lock().as_ref().and_then(|fe| {
        if fe.halted && !fe.effect.is_empty() {
            Some(fe.effect.clone())
        } else {
            None
        }
    });
    match halted_effect {
        Some(effect) => fallback_apply(&effect),
        None => fallback_restore(),
    }
    crate::tap::prune_extra_residents(&live_pids);
}

// -------------------------------------------------- safe DWM fallback ----

/// fnv tag of the engine bytes — keys the breaker record so a shipped
/// `ExplorerBlurMica` fix clears the ban without user action.
fn ebm_dll_tag() -> String {
    ebm_source_bytes()
        .map(|b| format!("{:016x}", fnv1a64(&b)))
        .unwrap_or_default()
}

/// EBM effect → `DWMWA_SYSTEMBACKDROP_TYPE` value reachable WITHOUT any
/// injected code — verified working cross-process on `CabinetWClass`.
/// The blur family has no DWM equivalent; it degrades to the system's own
/// acrylic backdrop rather than leaving the window bare.
fn fallback_backdrop(effect: &str) -> Option<crate::dwm::SystemBackdrop> {
    use crate::dwm::SystemBackdrop as B;
    Some(match effect {
        "mica" => B::MainWindow,
        "acrylic" | "blur" | "blurclear" => B::TransientWindow,
        "micaalt" => B::TabbedWindow,
        _ => return None,
    })
}

/// Applies the DWM-only material to every `CabinetWClass` — the breaker
/// fallback. Idempotent and per-hwnd tracked; a changed `want` reapplies
/// all live frames.
fn fallback_apply(effect: &str) {
    let Some(want) = fallback_backdrop(effect) else {
        return;
    };
    let cabinets = cabinet_windows();
    let mut guard = lock();
    let Some(fe) = guard.as_mut() else { return };
    let kind = match want {
        crate::dwm::SystemBackdrop::MainWindow => 2,
        crate::dwm::SystemBackdrop::TransientWindow => 3,
        crate::dwm::SystemBackdrop::TabbedWindow => 4,
        _ => 0,
    };
    let kind_changed = fe.fallback_kind != kind;
    fe.fallback_hwnds
        .retain(|h| cabinets.iter().any(|c| c.0 as usize == *h));
    let before = fe.fallback_hwnds.len();
    for hwnd in cabinets {
        let key = hwnd.0 as usize;
        if !kind_changed && fe.fallback_hwnds.contains(&key) {
            continue;
        }
        if crate::dwm::set_system_backdrop(hwnd, want).is_ok() {
            fe.fallback_hwnds.insert(key);
        }
    }
    fe.fallback_kind = kind;
    if kind_changed || fe.fallback_hwnds.len() != before {
        tracing::debug!(
            effect,
            hwnds = fe.fallback_hwnds.len(),
            "EBM fallback: DWM backdrop applied (engine banned on this build)"
        );
    }
}

/// Restores `DWMSBT_AUTO` on every hwnd the fallback touched — hwnds are
/// class-checked first so a recycled handle never gets a stray write.
fn fallback_restore() {
    let taken = {
        let mut guard = lock();
        match guard.as_mut() {
            Some(fe) if !fe.fallback_hwnds.is_empty() => {
                fe.fallback_kind = 0;
                std::mem::take(&mut fe.fallback_hwnds)
            }
            _ => return,
        }
    };
    for key in taken {
        let hwnd = crate::bindings::HWND(key as *mut core::ffi::c_void);
        let mut cls = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, windows_core::PWSTR(cls.as_mut_ptr()), 64) };
        if n <= 0 || String::from_utf16_lossy(&cls[..n as usize]) != "CabinetWClass" {
            continue;
        }
        let _ = crate::dwm::set_system_backdrop(hwnd, crate::dwm::SystemBackdrop::Auto);
    }
}

/// One-shot disarm for legacy residents: an older app build may have
/// left `fe_glass` armed inside explorer — its DWM/uxtheme detours would
/// fight the EBM extension over the same surfaces. `FESR` restores
/// originals; no-op when nothing is armed.
fn disarm_legacy() {
    for hwnd in crate::tap::resident_windows_all() {
        let _ = crate::tap::send_copydata(hwnd, CDS_FE_RESET, &[]);
    }
}

/// One-shot reset for the retired `CDES-FES1` mux wire. Older builds (and
/// an already-injected resident carrying a layout/clear wire) keep their
/// mux styling until told otherwise — `FELR` restores stock chrome. Sent
/// unconditionally on every apply: a pid that never got the wire ignores it.
fn disarm_xaml() {
    let residents = crate::tap::resident_windows_all();
    if !residents.is_empty() {
        tracing::debug!(
            residents = residents.len(),
            "FELR broadcast: retired mux wire reset"
        );
    }
    for hwnd in residents {
        let _ = crate::tap::send_copydata(hwnd, CDS_FE_LAYOUT_RESET, &[]);
    }
}

// ------------------------------------------------------------- public ----

/// Applies an EBM `effect` + flags + RGBA palettes.
///
/// EBM lifecycle rule: `config.ini` is read once per `LoadLibrary`, so a
/// real config change reaches an already-loaded pid only through
/// `EBMU`+`EBML` — a mid-session `FreeLibrary` whose detach races live
/// detours. That unload is therefore confined to the pids whose ini-
/// generation trails: `epoch` bumps only when effect/flags/rgba actually
/// change. An identical apply delivers nothing and never disturbs the
/// material.
/// Returns `false` when the deploy or the `config.ini` write failed so
/// settings can revert.
pub fn apply_effect(cfg: &FeGlassConfig) -> bool {
    tracing::debug!(
        effect = cfg.effect,
        clear_address = cfg.clear_address,
        clear_bar_bg = cfg.clear_bar_bg,
        clear_winui_bg = cfg.clear_winui_bg,
        show_line = cfg.show_line,
        "apply_effect"
    );
    // Every path ends here: legacy fe_glass disarm (old builds' detours
    // would fight EBM), the retired mux-wire reset (FELR — restores stock
    // chrome in residents still carrying a layout/clear wire), then
    // delivery to every covered pid.
    fn finish() {
        disarm_legacy();
        disarm_xaml();
        deliver_all();
    }
    if cfg.effect.is_empty() {
        return reset_ebm();
    }
    if effect_id(cfg.effect).is_none() {
        tracing::warn!(
            effect = cfg.effect,
            "ExplorerBlurMica apply failed — unknown effect id"
        );
        finish();
        return false;
    }
    // What the ini would actually change — `epoch` bumps only then, so an
    // identical apply never triggers an unload+reload.
    let ini_changed = {
        let guard = lock();
        match guard.as_ref() {
            Some(fe) => {
                fe.effect != cfg.effect
                    || fe.clear_address != cfg.clear_address
                    || fe.clear_bar_bg != cfg.clear_bar_bg
                    || fe.clear_winui_bg != cfg.clear_winui_bg
                    || fe.show_line != cfg.show_line
                    || fe.light_rgba != cfg.light_rgba
                    || fe.dark_rgba != cfg.dark_rgba
            }
            None => true,
        }
    };
    if !ini_changed && ARMED.load(Ordering::Acquire) {
        tracing::debug!("apply_effect: EBM config unchanged — material leg skipped");
        finish();
        return true;
    }
    let Some(dir) = residents_dir() else {
        tracing::warn!("ExplorerBlurMica apply failed — %LOCALAPPDATA% residents dir unavailable");
        finish();
        return false;
    };
    // Deploy + config.ini are committed BEFORE touching shared state: on
    // failure `fe.effect` still holds the last applied value and the ini
    // on disk still matches it — no rollback needed. `config.ini` sits in
    // the deployed DLL's own versioned dir — EBM reads its module dir.
    let Some(dll_path) = deploy_dll() else {
        finish();
        return false;
    };
    let ini_dir = dll_path.parent().unwrap_or(&dir).to_path_buf();
    if let Err(e) = write_config_ini(&ini_dir, cfg) {
        tracing::warn!(
            ini = %ini_dir.join("config.ini").display(),
            %e,
            "ExplorerBlurMica apply failed — config.ini write"
        );
        finish();
        return false;
    }
    {
        let mut guard = lock();
        let fe = guard.get_or_insert_with(Fe::default);
        fe.effect = cfg.effect.to_string();
        fe.clear_address = cfg.clear_address;
        fe.clear_bar_bg = cfg.clear_bar_bg;
        fe.clear_winui_bg = cfg.clear_winui_bg;
        fe.show_line = cfg.show_line;
        fe.light_rgba = cfg.light_rgba.to_string();
        fe.dark_rgba = cfg.dark_rgba.to_string();
        fe.dll_path = dll_path.display().to_string();
        fe.epoch += 1; // pids whose generation trails reload EBM once
        // The settle timer starts NOW — `deliver_all` holds the reload
        // until the ini burst quiets, collapsing it into one EBMU+EBML.
        fe.last_ini_change = Some(Instant::now());
        // A real config change is a fresh contract — clear the crash
        // breaker so a fixed config is not held guilty by an old engine.
        fe.strikes = 0;
        fe.halted = false;
    }
    // The persisted breaker outranks the fresh-contract reset: on THIS
    // Windows revision with THIS engine binary, EBM stays disarmed no
    // matter which effect is applied — the DllMain detours are the crash
    // surface and they install for every material. The DWM fallback still
    // delivers a usable backdrop while banned.
    let dll_tag = ebm_dll_tag();
    if breaker_banned(&dll_tag) {
        if let Some(fe) = lock().as_mut() {
            fe.halted = true;
            fe.delivered.clear();
        }
        ARMED.store(true, Ordering::Release);
        broadcast_ebm_unload();
        finish();
        tracing::warn!(
            effect = cfg.effect,
            build = %os_build_tag(),
            "EBM apply honored but kept disarmed — engine banned on this Windows build (DWM fallback active)"
        );
        return true;
    }
    // No ban — a record for an older build or engine is stale self-heal
    // bait; dropping it is the "world changed" retry.
    breaker_drop_if_stale(&dll_tag);
    ARMED.store(true, Ordering::Release);
    finish();
    tracing::info!(
        effect = cfg.effect,
        "ExplorerBlurMica armed via resident relay"
    );
    true
}

/// `EBMU` broadcast unloads the material everywhere and clears its
/// delivery bookkeeping; the retired mux wire gets its reset too so stock
/// chrome comes back with the stock material.
fn reset_ebm() -> bool {
    let residents = crate::tap::resident_windows_all();
    tracing::debug!(residents = residents.len(), "reset_ebm: EBMU broadcast");
    broadcast_ebm_unload();
    disarm_legacy();
    disarm_xaml();
    fallback_restore();
    if let Some(fe) = lock().as_mut() {
        fe.effect.clear();
        fe.delivered.clear();
        fe.delivered_at.clear();
        fe.first_seen.clear();
        fe.strikes = 0;
        fe.halted = false;
    }
    ARMED.store(false, Ordering::Release);
    true
}

/// `true` when the crash circuit breaker tripped — `EBML` no longer fans
/// out this session. Settings can surface this ("Explorer material paused
/// for stability") instead of silently showing a dead effect.
pub fn halted() -> bool {
    lock().as_ref().is_some_and(|fe| fe.halted)
}

/// `Windows Default` — full disarm: EBM unload + mux/legacy resets.
/// Used by shutdown; the settings Apply path goes through `apply_effect`.
pub fn reset() -> bool {
    reset_ebm()
}

/// Anchor-tick liveness: re-injects + re-delivers `EBML` to explorer
/// pids that appeared since the last pass (new cabinet process), whose
/// resident restarted, or whose config epoch trails. No wires fire when
/// nothing changed — no periodic Unadvise/Advise or LoadLibrary churn.
pub fn ensure() {
    if !is_armed() {
        return;
    }
    deliver_all();
}

/// App shutdown — unload EBM everywhere so Explorer returns to stock;
/// the deployed files stay for the next run.
pub fn shutdown() {
    let _ = reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_names_match_config_ini() {
        assert_eq!(effect_id("blur"), Some(0));
        assert_eq!(effect_id("acrylic"), Some(1));
        assert_eq!(effect_id("mica"), Some(2));
        assert_eq!(effect_id("blurclear"), Some(3));
        assert_eq!(effect_id("micaalt"), Some(4));
        assert_eq!(effect_id(""), None);
        assert_eq!(effect_id("bogus"), None);
    }

    #[test]
    fn fallback_maps_every_effect() {
        use crate::dwm::SystemBackdrop as B;
        assert_eq!(fallback_backdrop("mica"), Some(B::MainWindow));
        assert_eq!(fallback_backdrop("micaalt"), Some(B::TabbedWindow));
        assert_eq!(fallback_backdrop("acrylic"), Some(B::TransientWindow));
        assert_eq!(fallback_backdrop("blur"), Some(B::TransientWindow));
        assert_eq!(fallback_backdrop("blurclear"), Some(B::TransientWindow));
        assert_eq!(fallback_backdrop(""), None);
        assert_eq!(fallback_backdrop("bogus"), None);
    }

    #[test]
    fn rgba_parses_aarrggbb() {
        assert_eq!(parse_rgba("#A0DCDCDC"), Some((0xDC, 0xDC, 0xDC, 0xA0)));
        assert_eq!(parse_rgba("#78000000"), Some((0, 0, 0, 0x78)));
        assert_eq!(parse_rgba("FF0000"), Some((0xFF, 0, 0, 0xFF)));
        assert_eq!(parse_rgba("#xyz"), None);
        assert_eq!(parse_rgba(""), None);
    }

    #[test]
    fn config_ini_matches_upstream_layout() {
        let dir = std::env::temp_dir().join("coredesk_ebm_cfg_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = FeGlassConfig {
            effect: "acrylic",
            clear_address: true,
            clear_bar_bg: false,
            clear_winui_bg: true,
            show_line: true,
            light_rgba: "#A0DCDCDC",
            dark_rgba: "#78000000",
        };
        write_config_ini(&dir, &cfg).unwrap();
        // Identical contents are skipped on a second apply — an EBM-held
        // read handle must never turn into a sharing violation.
        write_config_ini(&dir, &cfg).unwrap();
        let ini = std::fs::read_to_string(dir.join("config.ini")).unwrap();
        assert!(ini.contains("[config]\r\neffect=1"));
        assert!(ini.contains("clearAddress=true"));
        assert!(ini.contains("clearBarBg=false"));
        assert!(ini.contains("clearWinUIBg=true"));
        assert!(ini.contains("showLine=true"));
        assert!(ini.contains("[light]\r\nr=220\r\ng=220\r\nb=220\r\na=160"));
        assert!(ini.contains("[dark]\r\nr=0\r\ng=0\r\nb=0\r\na=120"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
