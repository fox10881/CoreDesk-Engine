//! Compatibility report generator — the payload behind the post-crash "share
//! report" prompt and `coredesk-cli compat report`. One markdown file the
//! user can paste straight into the CoreDesk-feedback issue template:
//! build/UBR, the armed style surfaces, the `NoDiag` breaker state, recent
//! WER crash records for `explorer.exe` (faulting module + exception code —
//! the forensic half of every shell-styling incident), and the log tail.
//! Everything here is read-only: no registry writes, no resident traffic.

use std::path::PathBuf;
use windows_core::PCWSTR;

windows_core::link!("advapi32.dll" "system" fn RegGetValueW(hkey: isize, lpsubkey: PCWSTR, lpvalue: PCWSTR, dwflags: u32, pdwtype: *mut u32, pvdata: *mut core::ffi::c_void, pcbdata: *mut u32) -> i32);

const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize;
const DEBUG_SUBKEY: &str = crate::taskbar_styler::NODIAG_SUBKEY;

fn widestr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// REG_SZ reader — same `RegGetValueW` pattern as `taskbar_styler::reg_dword`.
fn reg_sz(root: isize, sub: &str, name: &str) -> Option<String> {
    const RRF_RT_REG_SZ: u32 = 0x02;
    let sub = widestr(sub);
    let name = widestr(name);
    let mut buf = [0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

fn localdata_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("CoreDeskEngine"))
}

/// Where `config.json` lives — the appdata layout first, then the portable
/// `<exe>\config\` layout so portable installs report their real surfaces too.
fn config_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from)?;
    let standard = appdata.join("CoreDeskEngine").join("config.json");
    if standard.exists() {
        return Some(standard);
    }
    let portable = std::env::current_exe()
        .ok()?
        .parent()?
        .join("config")
        .join("config.json");
    portable.exists().then_some(portable)
}

/// The style surfaces worth listing in a report — the same keys a maintainer
/// asks about first when a styling bug lands.
const SURFACE_KEYS: &[&str] = &[
    "taskbarStyle",
    "startMenuStyle",
    "notificationCenterStyle",
    "themeStyle",
    "feEffect",
    "backdrop",
    "clockStyle",
    "visualizerStyle",
    "taskbarWidgetStyle",
    "desktopEffect",
    "wallpaperPath",
];

fn surfaces_section() -> String {
    let mut out = String::new();
    let Some(path) = config_path() else {
        out.push_str("_(config.json not found)_\n");
        return out;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        out.push_str("_(config.json unreadable)_\n");
        return out;
    };
    let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&text) else {
        out.push_str("_(config.json unparseable)_\n");
        return out;
    };
    // The style surfaces live under the `settings` object.
    let cfg = cfg.get("settings").cloned().unwrap_or(cfg);
    let mut any = false;
    for key in SURFACE_KEYS {
        let Some(v) = cfg.get(*key) else { continue };
        let shown = match v {
            serde_json::Value::String(s) if s.is_empty() => "\"\" (Windows Default)".to_string(),
            serde_json::Value::String(s) => format!("\"{s}\""),
            other => other.to_string(),
        };
        // wallpaperPath is a local path — report presence, not the path.
        let shown = if *key == "wallpaperPath" {
            if v.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                "set".to_string()
            } else {
                "unset".to_string()
            }
        } else {
            shown
        };
        out.push_str(&format!("- `{key}`: {shown}\n"));
        any = true;
    }
    if !any {
        out.push_str("_(no style surfaces armed)_\n");
    }
    out
}

/// The `NoDiag` breaker state — armed means the crash protection tripped on
/// this OS revision and taskbar diagnostics are parked.
fn nodiag_section() -> String {
    let armed =
        crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiag") == Some(1);
    if !armed {
        return "- **NoDiag breaker**: not armed\n".to_string();
    }
    let build = crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagBuild")
        .map(|b| b.to_string())
        .unwrap_or_else(|| "?".into());
    let ubr = crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagUbr")
        .map(|b| b.to_string())
        .unwrap_or_else(|| "?".into());
    let dll = reg_sz(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagDll").unwrap_or_else(|| "?".into());
    format!(
        "- **NoDiag breaker**: ARMED — taskbar styling parked on build {build}.{ubr}, resident `{dll}`\n"
    )
}

/// The resident image currently deployed — the `coredesk_tap_v9_<hash>.dll`
/// filename already encodes its own image tag.
fn resident_image() -> String {
    if let Some(dir) = localdata_dir().map(|d| d.join("residents")) {
        let mut newest: Option<(std::time::SystemTime, String)> = None;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !(name.starts_with("coredesk_tap") && name.ends_with(".dll")) {
                    continue;
                }
                let mtime = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                    newest = Some((mtime, name));
                }
            }
        }
        if let Some((_, name)) = newest {
            return name;
        }
    }
    "not deployed".to_string()
}

/// Recent `Application Error` (1000) / WER (1001) events whose text mentions
/// `explorer.exe` or `coredesk` — `Faulting module name` + `Exception code`
/// are the forensic half of every shell-styling incident. `wevtutil` is
/// present on every supported Windows; text parsing beats a second binding.
fn wer_section() -> String {
    let mut out = String::from("```\n");
    let query = r#""*[System[(EventID=1000 or EventID=1001)]]""#;
    let result = std::process::Command::new("wevtutil")
        .args([
            "qe",
            "Application",
            "/q:",
            query,
            "/rd:true",
            "/f:text",
            "/c:200",
        ])
        .output();
    let blocks: Vec<String> = match result {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.split("\nEvent[")
                .enumerate()
                .filter(|(i, _)| *i > 0)
                .map(|(_, b)| format!("Event[{b}"))
                .filter(|b| {
                    let low = b.to_lowercase();
                    low.contains("explorer.exe") || low.contains("coredesk")
                })
                .take(15)
                .collect()
        }
        _ => Vec::new(),
    };
    if blocks.is_empty() {
        out.push_str("no recent explorer.exe / coredesk crash records found\n");
    } else {
        for b in &blocks {
            out.push_str(b.trim_end());
            out.push_str("\n\n");
        }
    }
    out.push_str("```\n");
    out
}

/// Last ~120 lines of the runtime log — enough context without the flood.
fn log_tail() -> String {
    let mut out = String::from("```\n");
    match localdata_dir()
        .map(|d| d.join("coredesk.log"))
        .and_then(|p| std::fs::read_to_string(p).ok())
    {
        Some(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(120);
            for l in &lines[start..] {
                out.push_str(l);
                out.push('\n');
            }
        }
        None => out.push_str("_(log unreadable)_\n"),
    }
    out.push_str("```\n");
    out
}

/// Builds the report text — shared by the CLI command and the in-app share
/// flow so both always produce the same shape.
pub fn render() -> String {
    let (build, ubr) = crate::taskbar_styler::current_build_ubr();
    let mut r = String::with_capacity(48 * 1024);
    r.push_str("# CoreDesk compatibility report\n\n");
    r.push_str(&format!("- **app**: {}\n", env!("CARGO_PKG_VERSION")));
    r.push_str(&format!("- **os**: Windows build {build}.{ubr}\n"));
    r.push_str(&format!("- **resident**: {}\n", resident_image()));
    r.push_str(&nodiag_section());
    r.push_str("\n## Armed style surfaces\n\n");
    r.push_str(&surfaces_section());
    r.push_str("\n## Recent shell crash records (WER)\n\n");
    r.push_str(&wer_section());
    r.push_str("\n## Log tail\n\n");
    r.push_str(&log_tail());
    r
}

/// Writes the report under `%LOCALAPPDATA%\CoreDeskEngine\reports\` and
/// returns its path.
pub fn write() -> Result<PathBuf, String> {
    let dir = localdata_dir().ok_or("no %LOCALAPPDATA%")?.join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("compat-report-{stamp}.md"));
    std::fs::write(&path, render()).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The incident key the share prompt de-duplicates on — one question per
/// (build, UBR, resident image) trip, so a fresh crash on a new build asks
/// again but the same lock never nags twice.
pub fn nodiag_incident_key() -> Option<String> {
    if crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiag") != Some(1) {
        return None;
    }
    let build = crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagBuild")
        .unwrap_or(0);
    let ubr =
        crate::taskbar_styler::reg_dword(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagUbr").unwrap_or(0);
    let dll = reg_sz(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagDll").unwrap_or_default();
    Some(format!("{build}.{ubr}.{dll}"))
}

/// `true` while the armed incident has not been answered yet — the prompt
/// marker lives next to the breaker values so it inherits the same lifetime.
pub fn nodiag_unanswered() -> bool {
    let Some(key) = nodiag_incident_key() else {
        return false;
    };
    reg_sz(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagAsked")
        .map(|a| a != key)
        .unwrap_or(true)
}

/// Builds the one-click GitHub issue URL — the whole report pre-filled in
/// `body`, so after Share the only thing left on the user is GitHub's own
/// Submit click (posting under their account can't be automated anyway).
/// The body is capped so the URL stays well inside server/browser limits.
pub fn share_issue_url() -> String {
    const MAX_BODY: usize = 6000;
    let mut body = render();
    if body.len() > MAX_BODY {
        let mut end = MAX_BODY;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str(
            "\n\n_(truncated — the full report is saved under \
             %LOCALAPPDATA%\\CoreDeskEngine\\reports\\)_\n",
        );
    }
    format!(
        "https://github.com/fox10881/CoreDesk-feedback/issues/new?title={}&body={}",
        urlencoded(&format!(
            "Compatibility report — CoreDesk {}",
            env!("CARGO_PKG_VERSION")
        )),
        urlencoded(&body)
    )
}

/// RFC 3986 unreserved set stays literal; everything else becomes `%XX`.
/// The report carries arbitrary log text (non-ASCII included), so this
/// works on raw UTF-8 bytes.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Records the incident as answered (share or dismiss — either counts).
pub fn nodiag_mark_answered() {
    let Some(key) = nodiag_incident_key() else {
        return;
    };
    set_reg_sz(HKEY_CURRENT_USER, DEBUG_SUBKEY, "NoDiagAsked", &key);
}

fn set_reg_sz(root: isize, sub: &str, name: &str, value: &str) {
    const KEY_WRITE: u32 = 0x2_0006;
    windows_core::link!("advapi32.dll" "system" fn RegCreateKeyExW(hkey: isize, lpsubkey: PCWSTR, reserved: u32, lpclass: PCWSTR, dwoptions: u32, samdesired: u32, lpsecurityattributes: *const core::ffi::c_void, phkresult: *mut isize, lpdwdisposition: *mut u32) -> i32);
    windows_core::link!("advapi32.dll" "system" fn RegSetValueExW(hkey: isize, lpvaluename: PCWSTR, reserved: u32, dwtype: u32, lpdata: *const u8, cbdata: u32) -> i32);
    windows_core::link!("advapi32.dll" "system" fn RegCloseKey(hkey: isize) -> i32);
    const REG_SZ: u32 = 1;
    let sub_w = widestr(sub);
    let mut hkey: isize = 0;
    if unsafe {
        RegCreateKeyExW(
            root,
            PCWSTR(sub_w.as_ptr()),
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
        return;
    }
    let name_w = widestr(name);
    let mut data: Vec<u16> = value.encode_utf16().collect();
    data.push(0);
    unsafe {
        let _ = RegSetValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            0,
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        );
        let _ = RegCloseKey(hkey);
    }
}
