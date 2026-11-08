//! ExplorerBlurMica relay — the vendored `ExplorerBlurMica.dll` is a COM
//! shell extension whose `DllMain` self-installs its hooks when loaded
//! into `explorer.exe` (it refuses other processes by returning FALSE).
//! FolderExtensions registration needs HKLM/admin, so instead of COM the
//! platform sends the DLL's path over `EBML` and we `LoadLibraryW` it
//! in-process — identical engine, zero admin, per-pid coverage wherever
//! a resident already lives. `EBMU` unloads it (`ExplorerBlurMica.Detach`
//! restores originals).

use crate::win32;
use core::sync::atomic::{AtomicIsize, Ordering};
use std::path::{Component, Path, PathBuf};
use windows_core::PCWSTR;

/// HMODULE of the EBM image this resident loaded — 0 when unloaded.
/// `DllMain` re-entry semantics make double-load dangerous, so the
/// handle itself is the guard.
static EBM_MODULE: AtomicIsize = AtomicIsize::new(0);

/// Decodes a `WM_COPYDATA` `EBML` payload into an aligned, NUL-terminated
/// UTF-16 buffer only when it names the canonical
/// `%LOCALAPPDATA%\CoreDeskEngine\residents\ebm-<16hex>\ExplorerBlurMica.dll`
/// location with no traversal segments.
fn validate_ebm_path(bytes: &[u8], expected_residents_dir: Option<&Path>) -> Option<Vec<u16>> {
    if bytes.len() < 4 || bytes.len() > 2048 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    // Strip trailing NULs (and require at least one trailing NUL in the wire).
    if units.last().copied() != Some(0) {
        return None;
    }
    while units.last().copied() == Some(0) {
        units.pop();
    }
    // Reject embedded NULs or empty strings.
    if units.is_empty() || units.contains(&0) {
        return None;
    }
    let text = String::from_utf16(&units).ok()?;
    let path = PathBuf::from(&text);
    if !path.is_absolute() {
        return None;
    }
    if !path.components().all(|c| {
        matches!(
            c,
            Component::Prefix(_) | Component::RootDir | Component::Normal(_)
        )
    }) {
        return None;
    }
    let file_name = path.file_name()?.to_str()?;
    if !file_name.eq_ignore_ascii_case("ExplorerBlurMica.dll") {
        return None;
    }
    let ebm_dir = path.parent()?;
    let ebm_dir_name = ebm_dir.file_name()?.to_str()?;
    let hex_part = ebm_dir_name.strip_prefix("ebm-")?;
    if hex_part.len() != 16 || !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let residents_dir = ebm_dir.parent()?;
    let expected = expected_residents_dir?;
    if !residents_dir
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        return None;
    }
    units.push(0);
    Some(units)
}

fn default_residents_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("CoreDeskEngine")
            .join("residents"),
    )
}

/// `EBML` payload = UTF-16 NUL-terminated path to `ExplorerBlurMica.dll`.
pub fn load(bytes: &[u8]) {
    if EBM_MODULE.load(Ordering::Acquire) != 0 {
        return;
    }
    let Some(wide) = validate_ebm_path(bytes, default_residents_dir().as_deref()) else {
        crate::tap_log!("ebm: rejected untrusted or malformed EBML path");
        return;
    };
    if !Path::new(&String::from_utf16_lossy(&wide[..wide.len() - 1])).is_file() {
        crate::tap_log!("ebm: validated path does not exist on disk");
        return;
    }
    let h = unsafe { win32::LoadLibraryW(PCWSTR(wide.as_ptr())) };
    if h.is_null() {
        crate::tap_log!("ebm: LoadLibraryW failed");
        return;
    }
    EBM_MODULE.store(h as isize, Ordering::Release);
    crate::tap_log!("ebm: loaded");
}

/// `EBMU` — unload the image; EBM's `DllMain` detach path restores the
/// windows it styled.
pub fn unload() {
    let h = EBM_MODULE.swap(0, Ordering::AcqRel);
    if h != 0 {
        unsafe {
            let _ = win32::FreeLibrary(h as win32::HMODULE);
        }
        crate::tap_log!("ebm: unloaded");
    }
}

/// Unload on our own teardown (`WM_TAP_UNLOAD` / `DllMain` detach) so a
/// disabled effect doesn't outlive the resident that armed it.
pub fn shutdown() {
    unload();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_bytes(s: &str, nul: bool) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        if nul {
            v.extend_from_slice(&0u16.to_le_bytes());
        }
        v
    }

    #[test]
    fn ebm_path_validation_enforces_nul_and_canonical_residents_dir() {
        let res = Path::new(r"C:\Users\alice\AppData\Local\CoreDeskEngine\residents");
        let ok_path = r"C:\Users\alice\AppData\Local\CoreDeskEngine\residents\ebm-eb336ff0dd8e4f8c\ExplorerBlurMica.dll";
        assert!(validate_ebm_path(&utf16_bytes(ok_path, true), Some(res)).is_some());
        // Missing NUL terminator must be rejected.
        assert!(validate_ebm_path(&utf16_bytes(ok_path, false), Some(res)).is_none());
        // Arbitrary DLL outside residents or with traversal must be rejected.
        let bad_traversal = r"C:\Users\alice\AppData\Local\CoreDeskEngine\residents\ebm-eb336ff0dd8e4f8c\..\..\evil.dll";
        assert!(validate_ebm_path(&utf16_bytes(bad_traversal, true), Some(res)).is_none());
        let bad_name =
            r"C:\Users\alice\AppData\Local\CoreDeskEngine\residents\ebm-eb336ff0dd8e4f8c\other.dll";
        assert!(validate_ebm_path(&utf16_bytes(bad_name, true), Some(res)).is_none());
    }
}
