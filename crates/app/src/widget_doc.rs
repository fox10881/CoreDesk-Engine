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

fn inject_csp(doc: String) -> String {
    let lower = doc.to_ascii_lowercase();
    if let Some(idx) = lower.find("<head>") {
        let insert_at = idx + "<head>".len();
        let mut out = String::with_capacity(doc.len() + WIDGET_CSP_META.len() + 1);
        out.push_str(&doc[..insert_at]);
        out.push_str(WIDGET_CSP_META);
        out.push_str(&doc[insert_at..]);
        out
    } else {
        format!("{WIDGET_CSP_META}{doc}")
    }
}

/// Builds the document a widget host navigates to for design `id` in
/// `category`: the resolved `.html` file with a strict CSP and locale
/// markers injected.
/// `None` when no file resolves (factory missing + no user override) or it
/// fails the size/UTF-8 checks — callers treat that like an unknown id.
pub(crate) fn build_widget_document(category: &str, id: &str) -> Option<String> {
    let file = coredesk_core::styles_fs::load_widget_document(category, id)?;
    if !file.contains("<!-- COREDESK_LOCALE -->") && !file.contains("i18n.js") {
        return Some(inject_csp(file));
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
