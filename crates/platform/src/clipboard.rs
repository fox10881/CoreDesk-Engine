//! The OLE clipboard as Explorer uses it: a `CF_HDROP` file list plus the shell's
//! `Preferred DropEffect` word that distinguishes 剪切 (move) from 复制 (copy).

use crate::bindings::*;
use crate::dragdrop;
use std::path::PathBuf;
use std::sync::OnceLock;

windows_core::link!("user32.dll" "system" fn OpenClipboard(hwndnewowner: HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EmptyClipboard() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetClipboardData(uformat: u32, hmem: HANDLE) -> HANDLE);
windows_core::link!("user32.dll" "system" fn CloseClipboard() -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GlobalAlloc(uflags: u32, dwbytes: usize) -> HGLOBAL);
windows_core::link!("kernel32.dll" "system" fn GlobalFree(hmem: HGLOBAL) -> HGLOBAL);

static PREFERRED_EFFECT_FORMAT: OnceLock<u16> = OnceLock::new();

/// Files currently on the clipboard and whether they were cut (`true`) or copied (`false`).
/// A missing `Preferred DropEffect` counts as copy, as in Explorer.
pub fn file_list() -> Option<(Vec<PathBuf>, bool)> {
    // SAFETY: plain OLE call; the returned object is released when dropped.
    let data = unsafe { OleGetClipboard() }.ok()?;
    if !dragdrop::has_hdrop(&data) {
        return None;
    }
    let paths = dragdrop::hdrop_paths(&data);
    if paths.is_empty() {
        return None;
    }
    let fmt = dragdrop::hglobal_format(dragdrop::registered_format(
        "Preferred DropEffect",
        &PREFERRED_EFFECT_FORMAT,
    ));
    let effect = dragdrop::hglobal_bytes(&data, &fmt)
        .filter(|b| b.len() >= 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(DROPEFFECT_COPY as u32);
    let cut = effect & DROPEFFECT_MOVE as u32 != 0 && effect & DROPEFFECT_COPY as u32 == 0;
    Some((paths, cut))
}

/// Cheap check for menu enabling: is there a file list on the clipboard?
pub fn has_file_list() -> bool {
    // SAFETY: plain OLE call.
    unsafe { OleGetClipboard() }
        .map(|d| dragdrop::has_hdrop(&d))
        .unwrap_or(false)
}

/// Empties the clipboard (Explorer does this after pasting cut files).
pub fn clear() {
    // SAFETY: plain OLE call; a null object empties the clipboard.
    unsafe {
        let _ = OleSetClipboard(None::<&IDataObject>);
    }
}

/// `GetClipboardSequenceNumber`: changes whenever any program changes the clipboard.
pub fn sequence() -> u32 {
    // SAFETY: plain FFI call.
    unsafe { GetClipboardSequenceNumber() }
}

/// `CF_UNICODETEXT` text on the clipboard — used by the compat-report
/// share flow, which can't fit the report into a GitHub issue URL.
/// `false` when the clipboard can't be opened (another app holds it).
pub fn set_text(text: &str) -> bool {
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: standard OpenClipboard / EmptyClipboard / SetClipboardData
    // protocol; on SetClipboardData success the system owns `mem`, so we
    // free only the failure paths.
    unsafe {
        if OpenClipboard(HWND::default()).0 == 0 {
            return false;
        }
        let _ = EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if mem.0.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        let dst = GlobalLock(mem);
        if dst.is_null() {
            let _ = GlobalFree(mem);
            let _ = CloseClipboard();
            return false;
        }
        core::ptr::copy_nonoverlapping(wide.as_ptr(), dst as *mut u16, wide.len());
        let _ = GlobalUnlock(mem);
        let ok = !SetClipboardData(CF_UNICODETEXT, mem).0.is_null();
        if !ok {
            let _ = GlobalFree(mem);
        }
        let _ = CloseClipboard();
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_text_bumps_the_clipboard_sequence() {
        let before = sequence();
        // Another app can momentarily hold the clipboard — retry briefly
        // instead of flaking.
        let mut ok = false;
        for _ in 0..50 {
            if set_text("coredesk clipboard test — تجربة") {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(ok, "clipboard stayed locked for a whole second");
        assert_ne!(sequence(), before);
    }
}
