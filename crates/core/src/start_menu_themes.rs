//! Start Menu style catalog — Windhawk `windows-11-start-menu-styler` themes
//! as user-editable files under `Styles\Start Menu style\` (two-layer
//! resolution: [`crate::styles_fs`]). `scripts/import-start-menu-themes.py`
//! regenerates the shipped set from the mod's C++ source.
//!
//! The settings UI receives only the `{id, label, custom}` option list; the
//! full targeting rules, style constants, resource variables and web-view
//! styles stay behind this module ([`theme`]). `id` is the style file stem;
//! `""` is Windows Default (the setting deleted, the stock Start Menu
//! restored).

use crate::styles_fs;

pub use crate::styles_fs::StyleOption;

/// Every selectable style — the file-backed catalog, sorted by label.
pub fn style_options() -> Vec<StyleOption> {
    styles_fs::style_options(styles_fs::START_MENU_DIR)
}

/// Whether `id` is selectable: `""` (Windows Default) or a style file in
/// either layer.
pub fn is_style_id(id: &str) -> bool {
    id.is_empty() || styles_fs::style_exists(styles_fs::START_MENU_DIR, id)
}

/// The theme's full converted payload — target styles, style constants,
/// theme-resource variables, web-view styles and the layout override, plus
/// the classic-layout variant where the mod ships one.
pub fn theme(id: &str) -> Option<serde_json::Value> {
    styles_fs::load_theme(styles_fs::START_MENU_DIR, id)
}

/// Serializes the theme into the line wire-format the injected
/// `coredesk_tap.dll` parses inside `StartMenuExperienceHost.exe` (the host
/// transports it with `WM_COPYDATA`):
///
/// ```text
/// CDES-SMS1
/// L <windowsDefault|forceNewLayout|newLayoutSideBySide>
/// T <target>
/// S <style>
/// ...
/// C                      (switch to the classic-variant section)
/// T <target> / S <style>
/// ...
/// R <key>\t<value>       (theme resource variables)
/// W <css-rule>           (webContentStyles — injected into the search
///                         WebView2 via `AddScriptToExecuteOnDocumentCreated`)
/// E
/// ```
///
/// `$constant` substitution is done here (longest-name-first, the mod's own
/// ordering), so the DLL only ever sees expanded style strings.
pub fn theme_wire(id: &str) -> Option<String> {
    Some(wire_from_payload(&theme(id)?))
}

/// The wire serializer against an already-loaded payload — kept separate
/// from file resolution so unit tests need no filesystem.
fn wire_from_payload(theme: &serde_json::Value) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str("CDES-SMS1\n");
    let layout = theme["startMenuLayout"]
        .as_str()
        .unwrap_or("windowsDefault");
    out.push_str("L ");
    out.push_str(layout);
    out.push('\n');

    // Constants: `name=value` pairs applied to every style/target string with
    // longest-name-first replacement order (LoadStyleConstants keeps them
    // sorted descending so longer names win; later duplicates override).
    let mut constants: Vec<(String, String)> = theme["styleConstants"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.starts_with("//"))
                .filter_map(|s| {
                    let (k, v) = s.split_once('=')?;
                    Some((k.trim().to_string(), v.trim().to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    // One expansion pass inside each constant value so a constant may
    // reference the ones defined before it (ParseStyleConstant applies the
    // constants loaded so far).
    for i in 0..constants.len() {
        let prior = constants[..i].to_vec();
        constants[i].1 = styles_fs::expand_constants(&constants[i].1, &prior);
    }
    constants.sort_by_key(|c| std::cmp::Reverse(c.0.len()));

    let emit_styles = |out: &mut String, styles: &[serde_json::Value]| {
        for entry in styles {
            let Some(target) = entry["target"].as_str() else {
                continue;
            };
            // `//` comments a target in the mod's settings format.
            if target.starts_with("//") {
                continue;
            }
            let target = styles_fs::expand_constants(target, &constants);
            let mut emitted_target = false;
            if let Some(arr) = entry["styles"].as_array() {
                for style in arr.iter().filter_map(|s| s.as_str()) {
                    let style = style.trim();
                    if style.is_empty() || style.starts_with("//") {
                        continue;
                    }
                    if !emitted_target {
                        out.push_str("T ");
                        out.push_str(&target.replace(['\t', '\n'], " "));
                        out.push('\n');
                        emitted_target = true;
                    }
                    let style = styles_fs::expand_constants(style, &constants);
                    out.push_str("S ");
                    out.push_str(&style.replace(['\t', '\n'], " "));
                    out.push('\n');
                }
            }
        }
    };

    if let Some(styles) = theme["targetStyles"].as_array() {
        emit_styles(&mut out, styles);
    }
    if let Some(classic) = theme["classicVariant"]["targetStyles"].as_array() {
        out.push_str("C\n");
        emit_styles(&mut out, classic);
    }
    if let Some(vars) = theme["themeResourceVariables"].as_array() {
        for var in vars.iter().filter_map(|v| v.as_str()) {
            if var.starts_with("//") {
                continue;
            }
            let var = styles_fs::expand_constants(var, &constants);
            out.push_str("R ");
            out.push_str(&var.replace(['\t', '\n'], " "));
            out.push('\n');
        }
    }
    // `webContentStyles`/`webViewTargetStyles` — the search-results
    // WebView2's injected CSS (both key names appear in the wild: the mods
    // use `webContentStyles`, the factory files were authored with
    // `webViewTargetStyles`). Each `{target, styles[]}` entry becomes one
    // `W` line carrying a complete CSS rule (`selector{decl;decl;…}`); the
    // resident concatenates them into the stylesheet it hands
    // `ICoreWebView2`.
    for key in ["webContentStyles", "webViewTargetStyles"] {
        let Some(entries) = theme[key].as_array() else {
            continue;
        };
        for entry in entries {
            let Some(selector) = entry["target"].as_str() else {
                continue;
            };
            if selector.starts_with("//") || selector.is_empty() {
                continue;
            }
            let Some(styles) = entry["styles"].as_array() else {
                continue;
            };
            let decls: Vec<String> = styles
                .iter()
                .filter_map(|s| s.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.starts_with("//"))
                .map(|s| styles_fs::expand_constants(s, &constants))
                .collect();
            if decls.is_empty() {
                continue;
            }
            out.push_str("W ");
            out.push_str(&selector.replace(['\t', '\n'], " "));
            out.push('{');
            out.push_str(&decls.join(";").replace(['\t', '\n'], " "));
            out.push_str("}\n");
        }
    }
    out.push_str("E\n");
    out
}

/// The option list as a JSON value for the settings state payload —
/// Windows Default first, then every resolvable file (`custom` marks
/// user-layer overrides).
pub fn style_options_json() -> serde_json::Value {
    let mut arr = vec![serde_json::json!({
        "id": "",
        "label": "Windows Default",
        "custom": false,
    })];
    arr.extend(style_options().into_iter().map(|o| {
        serde_json::json!({
            "id": o.id,
            "label": o.label,
            "custom": o.custom,
        })
    }));
    serde_json::Value::Array(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal payloads exercising the layout line, constants, the classic
    /// variant and resource variables.
    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "startMenuLayout": "windowsDefault",
            "styleConstants": ["CommonBgBrush=<WindhawkBlur BlurAmount=\"18\" TintColor=\"#25323232\"/>"],
            "targetStyles": [
                { "target": "Border#AcrylicBorder", "styles": ["Background:=$CommonBgBrush"] },
                { "target": "Grid", "styles": ["Margin=0"] }
            ],
            "classicVariant": {
                "targetStyles": [{ "target": "Border#Classic", "styles": ["Visibility=Collapsed"] }]
            },
            "themeResourceVariables": ["Overlay@Light=#40FFFFFF"]
        })
    }

    #[test]
    fn theme_wire_serializes_rules_resources_and_layout() {
        let wire = wire_from_payload(&fixture());
        assert!(wire.starts_with("CDES-SMS1\n"));
        assert!(wire.ends_with("E\n"));
        assert!(wire.contains("L windowsDefault\n"));
        assert!(wire.contains("T Border#AcrylicBorder\n"));
        assert!(!wire.contains("$CommonBgBrush"));
        assert!(wire.contains("S Background:=<WindhawkBlur"));
        assert!(wire.contains("\nC\n"));
        assert!(wire.contains("T Border#Classic\n"));
        assert!(wire.contains("R Overlay@Light=#40FFFFFF\n"));
    }

    #[test]
    fn theme_wire_serializes_webcontent_styles() {
        let mut doc = fixture();
        doc["webContentStyles"] = serde_json::json!([
            { "target": "html", "styles": ["background: transparent !important", "border: none !important"] },
            { "target": "//comment", "styles": ["ignored: yes"] },
            { "target": ".suggContainer", "styles": ["background-color: rgba(0,0,0,.5) !important"] }
        ]);
        let wire = wire_from_payload(&doc);
        assert!(
            wire.contains("W html{background: transparent !important;border: none !important}\n")
        );
        assert!(wire.contains("W .suggContainer{background-color: rgba(0,0,0,.5) !important}\n"));
        assert!(!wire.contains("W //"));
    }

    #[test]
    fn shipped_styles_serialize() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../styles");
        if !root.is_dir() {
            return;
        }
        let doc = crate::styles_fs::load_in(
            &root,
            None,
            styles_fs::START_MENU_DIR,
            "TranslucentStartMenu",
        )
        .expect("shipped TranslucentStartMenu");
        let wire = wire_from_payload(&doc);
        assert!(wire.starts_with("CDES-SMS1\n"));
        assert!(wire.contains("T Border#AcrylicBorder\n"));
        assert!(wire.contains("\nC\n"));
        let liquid =
            crate::styles_fs::load_in(&root, None, styles_fs::START_MENU_DIR, "LiquidGlass")
                .unwrap();
        assert_eq!(liquid["startMenuLayout"].as_str(), Some("forceNewLayout"));
    }
}
