//! Start-Menu search `WebView2` CSS injection — the `W` section of the
//! `CDES-SMS1` wire.
//!
//! The search-results pane inside `StartMenuExperienceHost.exe` is a hosted
//! `Microsoft.UI.Xaml.Controls.WebView2` (from the CBS `Microsoft.UI.Xaml`
//! package SMEH depends on). Once the visual-tree watcher reports the
//! element we walk handle → `IInspectable` → `IWebView2` → its
//! `CoreWebView2` property → the classic COM `ICoreWebView2`, then
//! `AddScriptToExecuteOnDocumentCreated` + `ExecuteScript` a small injector
//! that installs a `<style>` node (a `MutationObserver` keeps it applied
//! across the SPA's DOM rewrites).
//!
//! Interfaces are hand-declared — `get_CoreWebView2` is method 0 of the
//! control's `IWebView2` (vtable slot 6), and `ICoreWebView2` only needs the
//! classic COM slots up to `ExecuteScript`. All state stays in the calling
//! engine (`styler_engine` instantiates once per XAML framework); this
//! module is pure COM plumbing.

#![allow(non_snake_case, non_camel_case_types, clippy::all)]

use crate::bindings::{IXamlDiagnostics, InstanceHandle};
use windows_core::{GUID, HRESULT, IInspectable, Interface, PCWSTR};

const IID_ADD_SCRIPT_HANDLER: GUID = GUID::from_u128(0xb99369f3_9b11_44b5_bc6f_8e7895fcea17);
const IID_EXEC_SCRIPT_HANDLER: GUID = GUID::from_u128(0x49511172_cc67_4bca_9923_137112f4c4f8);

// `Microsoft.UI.Xaml.Controls.IWebView2` — IID from the CBS
// `Microsoft.UI.Xaml.winmd` that `StartMenuExperienceHost` depends on
// (`Microsoft.UI.Xaml.CBS` package dependency); `get_CoreWebView2` is
// method 0 of the interface.
windows_core::imp::define_interface!(
    IMuxWebView2,
    IMuxWebView2_Vtbl,
    0x2b2d49c2_997c_5069_a8f0_9b84_cd7e624b
);
#[repr(C)]
pub struct IMuxWebView2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    /// `get_CoreWebView2` — method 0 of the interface.
    pub CoreWebView2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl IMuxWebView2 {
    fn core_webview2(&self) -> windows_core::Result<IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CoreWebView2)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}

// `ICoreWebView2` — the classic COM interface the WinRT `CoreWebView2`
// object also exposes (the WebView2 loader's object implements both).
// Only the vtable slots up to `ExecuteScript` are declared; nothing past
// them is ever called.
windows_core::imp::define_interface!(
    ICoreWebView2,
    ICoreWebView2_Vtbl,
    0x76ececcb_2354_499b_9001_9ad6f89cc18c
);
type UnusedSlot = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    *mut core::ffi::c_void,
) -> windows_core::HRESULT;
#[repr(C)]
pub struct ICoreWebView2_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Settings: UnusedSlot,
    pub Source: UnusedSlot,
    pub Navigate: UnusedSlot,
    pub NavigateToString: UnusedSlot,
    pub AddScriptToExecuteOnDocumentCreated: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        PCWSTR,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RemoveScriptToExecuteOnDocumentCreated: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        PCWSTR,
    ) -> windows_core::HRESULT,
    pub ExecuteScript: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        PCWSTR,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

// ============================================================================
// Completed-handler COM object — one vtable shape serves both callbacks
// (each is `Invoke(HRESULT, PCWSTR)`). The AddScript flavour forwards the
// issued script id to `record` so a later reset can remove it; the
// ExecuteScript flavour ignores the result JSON.
// ============================================================================

#[repr(C)]
struct HandlerVtbl {
    query_interface: unsafe extern "system" fn(
        *mut Handler,
        *const GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    add_ref: unsafe extern "system" fn(*mut Handler) -> u32,
    release: unsafe extern "system" fn(*mut Handler) -> u32,
    invoke: unsafe extern "system" fn(*mut Handler, HRESULT, PCWSTR) -> windows_core::HRESULT,
}
#[repr(C)]
struct Handler {
    vtbl: &'static HandlerVtbl,
    refs: std::sync::atomic::AtomicU32,
    handle: u64,
    record: Option<fn(u64, String)>,
}

static HANDLER_VTBL: HandlerVtbl = HandlerVtbl {
    query_interface: handler_qi,
    add_ref: handler_addref,
    release: handler_release,
    invoke: handler_invoke,
};

fn new_handler(handle: u64, record: Option<fn(u64, String)>) -> *mut Handler {
    Box::into_raw(Box::new(Handler {
        vtbl: &HANDLER_VTBL,
        refs: std::sync::atomic::AtomicU32::new(1),
        handle,
        record,
    }))
}
unsafe extern "system" fn handler_qi(
    this: *mut Handler,
    iid: *const GUID,
    out: *mut *mut core::ffi::c_void,
) -> windows_core::HRESULT {
    unsafe {
        let iid = &*iid;
        if *iid == windows_core::IUnknown::IID
            || *iid == IID_ADD_SCRIPT_HANDLER
            || *iid == IID_EXEC_SCRIPT_HANDLER
        {
            handler_addref(this);
            *out = this.cast();
            return windows_core::HRESULT(0);
        }
        *out = core::ptr::null_mut();
        windows_core::HRESULT(0x8000_4002u32 as i32) // E_NOINTERFACE
    }
}
unsafe extern "system" fn handler_addref(this: *mut Handler) -> u32 {
    unsafe { &*this }
        .refs
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1
}
unsafe extern "system" fn handler_release(this: *mut Handler) -> u32 {
    unsafe {
        let left = (&*this)
            .refs
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed)
            - 1;
        if left == 0 {
            drop(Box::from_raw(this));
        }
        left
    }
}
unsafe extern "system" fn handler_invoke(
    this: *mut Handler,
    _hr: HRESULT,
    arg: PCWSTR,
) -> windows_core::HRESULT {
    unsafe {
        let h = &*this;
        if let Some(record) = h.record {
            let id = arg.to_string().unwrap_or_default();
            if !id.is_empty() {
                record(h.handle, id);
            }
        }
        windows_core::HRESULT(0)
    }
}

// ============================================================================
// Plumbing — element handle → ICoreWebView2, inject / uninject.
// ============================================================================

/// Element handle → the control's `ICoreWebView2`. `None` when the element
/// isn't the CBS `WebView2`, or its `CoreWebView2` isn't initialized yet
/// (the caller retries on later visual-tree traffic).
pub fn core_webview2(diagnostics: &IXamlDiagnostics, handle: u64) -> Option<ICoreWebView2> {
    let obj = unsafe { diagnostics.GetIInspectableFromHandle(InstanceHandle(handle)) }.ok()?;
    let wv = Interface::cast::<IMuxWebView2>(&obj).ok()?;
    let cwv_obj = wv.core_webview2().ok()?;
    Interface::cast::<ICoreWebView2>(&cwv_obj).ok()
}

/// Install `css` — `AddScriptToExecuteOnDocumentCreated` (survives
/// navigations; the returned script id reaches `record` when the async
/// call completes) plus an immediate `ExecuteScript` for the document
/// already loaded. `true` when at least one call was accepted.
pub fn inject(
    cwv: &ICoreWebView2,
    handle: u64,
    css: &str,
    record: fn(u64, String),
) -> bool {
    let js = windows_core::HSTRING::from(injector_js(css));
    let add = new_handler(handle, Some(record));
    let hr_add = unsafe {
        (Interface::vtable(cwv).AddScriptToExecuteOnDocumentCreated)(
            Interface::as_raw(cwv),
            PCWSTR::from_raw(js.as_ptr()),
            add.cast(),
        )
    };
    unsafe { handler_release(add) };
    let exec = new_handler(handle, None);
    let hr_exec = unsafe {
        (Interface::vtable(cwv).ExecuteScript)(
            Interface::as_raw(cwv),
            PCWSTR::from_raw(js.as_ptr()),
            exec.cast(),
        )
    };
    unsafe { handler_release(exec) };
    hr_add.is_ok() || hr_exec.is_ok()
}

/// Un-inject — blank the live `<style>` node and drop the document-created
/// script (`id` may still be empty while its AddScript callback is in
/// flight; the style node clear covers that window).
pub fn uninject(cwv: &ICoreWebView2, script_id: &str) {
    let blank = windows_core::HSTRING::from(injector_js(""));
    let exec = new_handler(0, None);
    let _ = unsafe {
        (Interface::vtable(cwv).ExecuteScript)(
            Interface::as_raw(cwv),
            PCWSTR::from_raw(blank.as_ptr()),
            exec.cast(),
        )
    };
    unsafe { handler_release(exec) };
    if !script_id.is_empty() {
        let id = windows_core::HSTRING::from(script_id);
        let _ = unsafe {
            (Interface::vtable(cwv).RemoveScriptToExecuteOnDocumentCreated)(
                Interface::as_raw(cwv),
                PCWSTR::from_raw(id.as_ptr()),
            )
        };
    }
}

// ============================================================================
// Injector JS — one `<style id="cde-css">` kept in sync by a
// MutationObserver (the search pane is a SPA that rewrites its DOM).
// ============================================================================

pub fn injector_js(css: &str) -> String {
    format!(
        "(()=>{{const c={};const a=()=>{{let e=document.getElementById('cde-css');\
         if(!e){{e=document.createElement('style');e.id='cde-css';\
         (document.head||document.documentElement).appendChild(e);}}\
         if(e.textContent!==c)e.textContent=c;}};\
         try{{a();new MutationObserver(a).observe(document.documentElement,\
         {{childList:true,subtree:true}});}}catch(_){{}}}})();",
        js_string(css)
    )
}

/// JSON-encode `s` into a JS string literal.
fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
