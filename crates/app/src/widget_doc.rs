//! Widget design documents — the file-backed WebView2 payloads behind the
//! clock and audio-visualizer widgets.
//!
//! Each design is a self-contained `<id>.html` resolved through the
//! two-layer `styles_fs` catalog ([`styles_fs::CLOCK_WIDGET_DIR`] /
//! [`styles_fs::VISUALIZER_WIDGET_DIR`]): a factory file next to the exe,
//! a user file under `%APPDATA%\CoreDeskEngine\Styles` shadowing it. The
//! host `NavigateToString`s [`build_widget_document`]'s output — the file
//! bytes with the `<!-- COREDESK_LOCALE -->` and
//! `<script src="i18n.js"></script>` markers (when present) swapped for the
//! live locale payload, so a mod file opts into localization by keeping
//! the markers.
//!
//! Inbound page→host messages run through [`widget_message_allowed`]'s
//! allowlist: a mod file is user-authored JavaScript, and the settings
//! message channel it shares with the settings UI must not be driven by
//! anything but the widget's own ready handshake.

use crate::clock_host::I18N_JS;

const WIDGET_CSP_META: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src 'none'; connect-src 'none'; frame-src 'none';\">";

/// rAF de-freezer injected into Visualizer widget documents (installed *before*
/// the eco shim so eco's requests stay tracked): WebView2's runtime can stop
/// scheduling `requestAnimationFrame` while the widget's own `resize` handler
/// has already wiped the canvas — the widget then renders nothing forever
/// because the pending rAF id stays non-zero and the page's own `!raf` re-arm
/// guard never trips. The wrapper tracks every outstanding callback; a
/// `setInterval` watchdog (timers keep running when rAF is frozen) cancels any
/// callback stalled >250 ms and runs it synchronously, so the paint loop
/// re-arms itself and the canvas repaints even while Chromium still believes
/// the surface should stay idle.
const VIZ_RAF_SHIM: &str = "<script>(()=>{const realRaf=window.requestAnimationFrame.bind(window),realCancel=window.cancelAnimationFrame.bind(window),pending=new Map();window.requestAnimationFrame=cb=>{let id=0;const run=now=>{pending.delete(id);cb(now);};id=realRaf(run);pending.set(id,{run:run,at:performance.now()});return id;};window.cancelAnimationFrame=id=>{pending.delete(id);realCancel(id);};setInterval(()=>{if(!pending.size)return;const now=performance.now();for(const[id,p]of pending){if(now-p.at>250){pending.delete(id);realCancel(id);try{p.run(now);}catch(_){}}}},125);})();</script>";

/// Eco-mode 30 FPS `requestAnimationFrame` throttle injected into Visualizer
/// widget documents: when `vizTheme` carries `"eco": true`, `requestAnimationFrame`
/// paces callbacks at ~33.3 ms (30 FPS); when `"eco": false`, it runs at native
/// 60 FPS with zero overhead.
const VIZ_ECO_SHIM: &str = "<script>(()=>{const origRaf=window.requestAnimationFrame.bind(window);let eco=false,last=0,attached=null;const handler=e=>{const m=e.data||{};if(m.t==='vizTheme'&&m.eco!==undefined)eco=!!m.eco;};const attach=()=>{try{const wv=window.chrome&&window.chrome.webview;if(wv&&wv!==attached){attached=wv;wv.addEventListener('message',handler);}}catch(_){attached=null;}setTimeout(attach,attached?500:10);};attach();window.requestAnimationFrame=cb=>{if(!eco)return origRaf(cb);return origRaf(now=>{if(now-last<32){setTimeout(()=>window.requestAnimationFrame(cb),Math.max(1,33-(now-last)));return;}last=now;cb(now);});};})();</script>";

fn inject_csp(doc: String, is_visualizer: bool) -> String {
    let extra = if is_visualizer {
        format!("{WIDGET_CSP_META}{VIZ_RAF_SHIM}{VIZ_ECO_SHIM}")
    } else {
        WIDGET_CSP_META.to_string()
    };
    let lower = doc.to_ascii_lowercase();
    if let Some(idx) = lower.find("<head>") {
        let insert_at = idx + "<head>".len();
        let mut out = String::with_capacity(doc.len() + extra.len() + 1);
        out.push_str(&doc[..insert_at]);
        out.push_str(&extra);
        out.push_str(&doc[insert_at..]);
        out
    } else {
        format!("{extra}{doc}")
    }
}

/// Builds the document a widget host navigates to for design `id` in
/// `category`: the resolved `.html` file with a strict CSP and locale
/// markers injected.
/// `None` when no file resolves (factory missing + no user override) or it
/// fails the size/UTF-8 checks — callers treat that like an unknown id.
pub(crate) fn build_widget_document(category: &str, id: &str) -> Option<String> {
    let is_viz = category == coredesk_core::styles_fs::VISUALIZER_WIDGET_DIR;
    let file = coredesk_core::styles_fs::load_widget_document(category, id)?;
    if !file.contains("<!-- COREDESK_LOCALE -->") && !file.contains("i18n.js") {
        return Some(inject_csp(file, is_viz));
    }
    let payload = coredesk_core::i18n::ui_payload()
        .to_string()
        .replace('<', "\\u003c");
    Some(inject_csp(
        file.replace(
            "<!-- COREDESK_LOCALE -->",
            &format!("<script>window.COREDESK_LOCALE={payload};</script>"),
        )
        .replace(
            "<script src=\"i18n.js\"></script>",
            &format!("<script>{I18N_JS}</script>"),
        ),
        is_viz,
    ))
}

/// The widget message allowlist: only `{"type": <allowed>}` reaches the
/// settings channel. Widget pages speak exactly one type (`clockReady` /
/// `vizReady`); anything else a modded document posts is dropped and
/// logged once per document load.
pub(crate) fn widget_message_allowed(json: &str, allowed: &[&str]) -> bool {
    let ok = serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v["type"].as_str().map(str::to_owned))
        .is_some_and(|t| allowed.contains(&t.as_str()));
    if !ok {
        tracing::debug!(json, "widget page message dropped by allowlist");
    }
    ok
}
