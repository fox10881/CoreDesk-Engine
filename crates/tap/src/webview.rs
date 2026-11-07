//! Start-Menu search `WebView2` CSS injection — the `W` section of the
//! `CDES-SMS1` wire.
//!
//! The search-results pane inside `SearchHost.exe`/`StartMenuExperienceHost.exe`
//! is a `WebView2Standalone.Controls.WebView2` element (the `WebView2Standalone`
//! package, hosted under `HostedWebView2Control`). Once the visual-tree watcher
//! reports the element we walk handle → `IInspectable` → `IWebView2` → its
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

// `WebView2Standalone::Controls::IWebView2` — the Cortana search pane's
// `WebView2Standalone.Controls.WebView2` element (same IID the Windhawk
// start-menu-styler mod queries); `get_CoreWebView2` is method 0.
windows_core::imp::define_interface!(
    IStandaloneWebView2,
    IStandaloneWebView2_Vtbl,
    0x59c47e46_cc96_525f_a17c_2c213f988447
);
#[repr(C)]
pub struct IStandaloneWebView2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    /// `get_CoreWebView2` — method 0.
    pub CoreWebView2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    /// `get_CoreWebView2Controller` — method 1.
    _m1: UnusedSlot,
    /// `EnsureCoreWebView2Async` — method 2.
    _m2: UnusedSlot,
    /// `ExecuteScriptAsync(HSTRING, IAsyncOperation<HSTRING>**)` — method 3.
    pub ExecuteScriptAsync: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    /// `get_Source` … `Close` — methods 4-14.
    _m4: UnusedSlot,
    _m5: UnusedSlot,
    _m6: UnusedSlot,
    _m7: UnusedSlot,
    _m8: UnusedSlot,
    _m9: UnusedSlot,
    _m10: UnusedSlot,
    _m11: UnusedSlot,
    _m12: UnusedSlot,
    _m13: UnusedSlot,
    _m14: UnusedSlot,
    /// `add_NavigationCompleted(handler, event_token*)` — method 15.
    pub NavigationCompleted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    /// `remove_NavigationCompleted(event_token)` — method 16.
    pub RemoveNavigationCompleted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i64,
    ) -> windows_core::HRESULT,
}
impl IStandaloneWebView2 {
    /// `ExecuteScriptAsync` — fire-and-forget: the returned operation is
    /// released immediately (the style node install is idempotent).
    fn exec_script(&self, js: &windows_core::HSTRING) -> windows_core::Result<()> {
        unsafe {
            let mut op = core::ptr::null_mut();
            let hr = (windows_core::Interface::vtable(self).ExecuteScriptAsync)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(js),
                &mut op,
            );
            release_raw(op);
            hr.ok()
        }
    }

    /// `add_NavigationCompleted` → `EventRegistrationToken`.
    fn add_navigation_completed(
        &self,
        handler: &crate::bindings::TypedEventHandler<IInspectable, IInspectable>,
    ) -> windows_core::Result<i64> {
        unsafe {
            let mut token = 0i64;
            (windows_core::Interface::vtable(self).NavigationCompleted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(handler),
                &mut token,
            )
            .map(|| token)
        }
    }

    fn remove_navigation_completed(&self, token: i64) {
        unsafe {
            let _ = (windows_core::Interface::vtable(self).RemoveNavigationCompleted)(
                windows_core::Interface::as_raw(self),
                token,
            );
        }
    }
}

/// Release a raw COM pointer returned through an out-param.
unsafe fn release_raw(p: *mut core::ffi::c_void) {
    if !p.is_null() {
        unsafe {
            let vtbl = *(p as *const *const windows_core::IUnknown_Vtbl);
            ((*vtbl).Release)(p as _);
        }
    }
}

// `Microsoft.UI.Xaml.Controls.IWebView2` — fallback for WinUI-flavoured
// `WebView2` elements (File Explorer etc.); `get_CoreWebView2` is
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

/// A resolved WebView element — the standalone Cortana control scripts
/// through `ExecuteScriptAsync` directly on the element (its WinRT
/// `CoreWebView2` doesn't QI to the classic `ICoreWebView2`, which is why
/// Windhawk uses the element's own method); other WebView2 surfaces go
/// through `CoreWebView2` → classic `ICoreWebView2`.
pub enum WebView {
    Standalone(IStandaloneWebView2),
    Core(ICoreWebView2),
}

/// What `inject` left behind — enough to fully undo it.
#[derive(Clone)]
pub struct InjectState {
    /// Classic path: `AddScriptToExecuteOnDocumentCreated` id ("" until
    /// the async id lands via `record`).
    pub script_id: String,
    /// Standalone path: `NavigationCompleted` token.
    pub nav_token: Option<i64>,
    /// Keep-alive for the nav handler (the control also holds a ref).
    _handler: Option<crate::bindings::TypedEventHandler<IInspectable, IInspectable>>,
}

/// Element handle → `WebView`. `None` when the element isn't a WebView2 or
/// its core isn't initialized yet (the caller retries on later traffic).
pub fn resolve(diagnostics: &IXamlDiagnostics, handle: u64) -> Option<WebView> {
    let obj = unsafe { diagnostics.GetIInspectableFromHandle(InstanceHandle(handle)) }
        .inspect_err(|e| crate::tap_log!("webview: GetIInspectableFromHandle hr={e:?}"))
        .ok()?;
    // The Cortana search pane's `WebView2Standalone.Controls.WebView2` is
    // the primary surface — `ExecuteScriptAsync` is always present on it,
    // so no `CoreWebView2`-initialized gate is needed.
    if let Ok(wv) = Interface::cast::<IStandaloneWebView2>(&obj) {
        return Some(WebView::Standalone(wv));
    }
    // Other WebView2 elements (mux flavour) — `CoreWebView2` → classic
    // `ICoreWebView2`; `None` while the core is still initializing.
    if let Ok(wv) = Interface::cast::<IMuxWebView2>(&obj) {
        let cwv_obj = wv
            .core_webview2()
            .inspect_err(|e| crate::tap_log!("webview: get_CoreWebView2 hr={e:?}"))
            .ok()?;
        return Interface::cast::<ICoreWebView2>(&cwv_obj)
            .inspect_err(|e| crate::tap_log!("webview: ICoreWebView2 cast hr={e:?}"))
            .ok()
            .map(WebView::Core);
    }
    // `HostedWebView2Control` wraps the real `WebView2Standalone.Controls.
    // WebView2` in a subtree the diagnostics session never reports — the
    // wrapper itself QIs for neither control interface, so walk its visual
    // subtree (in-proc; callers run on the element's UI thread).
    let root = Interface::cast::<crate::bindings::DependencyObject>(&obj)
        .inspect_err(|e| crate::tap_log!("webview: DependencyObject cast hr={e:?}"))
        .ok()?;
    let mut stack = vec![root];
    let mut budget = 512usize;
    while let Some(cur) = stack.pop() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        if let Ok(wv) = cur.cast::<IStandaloneWebView2>() {
            return Some(WebView::Standalone(wv));
        }
        if let Ok(n) = crate::bindings::VisualTreeHelper::GetChildrenCount(&cur) {
            for i in 0..n.min(64) {
                if let Ok(c) = crate::bindings::VisualTreeHelper::GetChild(&cur, i) {
                    stack.push(c);
                }
            }
        }
    }
    None
}

/// Install `css`. Standalone: `ExecuteScriptAsync` now + on every
/// `NavigationCompleted` (re-navigation drops the style node — the mod
/// re-applies the same way). Classic: `AddScriptToExecuteOnDocumentCreated`
/// + immediate `ExecuteScript`. `None` when nothing was accepted.
pub fn inject(
    webview: &WebView,
    handle: u64,
    css: &str,
    record: fn(u64, String),
) -> Option<InjectState> {
    match webview {
        WebView::Standalone(wv) => {
            let js = windows_core::HSTRING::from(injector_js(css));
            wv.exec_script(&js).ok()?;
            let js2 = js.clone();
            let handler = crate::bindings::TypedEventHandler::<IInspectable, IInspectable>::new(
                move |sender, _args| {
                    if let Some(s) = sender.cloned()
                        && let Ok(w) = s.cast::<IStandaloneWebView2>()
                    {
                        let _ = w.exec_script(&js2);
                    }
                    Ok(())
                },
            );
            let nav_token = wv
                .add_navigation_completed(&handler)
                .inspect_err(|e| crate::tap_log!("webview: add_NavigationCompleted hr={e:?}"))
                .ok();
            Some(InjectState {
                script_id: String::new(),
                nav_token,
                _handler: Some(handler),
            })
        }
        WebView::Core(cwv) => {
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
            (hr_add.is_ok() || hr_exec.is_ok()).then(|| InjectState {
                script_id: String::new(),
                nav_token: None,
                _handler: None,
            })
        }
    }
}

/// Un-inject — drop the nav subscription / document-created script and
/// blank the live `<style>` node (covers the window where the AddScript
/// id hasn't landed yet).
pub fn uninject(webview: &WebView, injected: &InjectState) {
    let blank = windows_core::HSTRING::from(injector_js(""));
    match webview {
        WebView::Standalone(wv) => {
            if let Some(token) = injected.nav_token {
                wv.remove_navigation_completed(token);
            }
            let _ = wv.exec_script(&blank);
        }
        WebView::Core(cwv) => {
            let exec = new_handler(0, None);
            let _ = unsafe {
                (Interface::vtable(cwv).ExecuteScript)(
                    Interface::as_raw(cwv),
                    PCWSTR::from_raw(blank.as_ptr()),
                    exec.cast(),
                )
            };
            unsafe { handler_release(exec) };
            if !injected.script_id.is_empty() {
                let id = windows_core::HSTRING::from(&*injected.script_id);
                let _ = unsafe {
                    (Interface::vtable(cwv).RemoveScriptToExecuteOnDocumentCreated)(
                        Interface::as_raw(cwv),
                        PCWSTR::from_raw(id.as_ptr()),
                    )
                };
            }
        }
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
