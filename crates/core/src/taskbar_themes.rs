//! Taskbar style catalog — Windhawk `windows-11-taskbar-styler` themes as
//! user-editable files under `Styles\Taskbar style\` (two-layer resolution:
//! [`crate::styles_fs`]). `scripts/import-taskbar-themes.py` regenerates the
//! shipped set from the mod's C++ source.
//!
//! The settings UI receives only the `{id, label, custom}` option list; the
//! full targeting rules, style constants and theme-resource variables stay
//! behind this module ([`theme`]). `id` is the style file stem; `""` is
//! Windows Default (the stock taskbar restored).
//!
//! `Squircle` is auto-resolved by the mod against OS feature `variantFeature`
//! (ExtendedModeAEPForTaskbar): when enabled, the `weatherVariant` payload
//! applies instead of the base. `resolve` picks the concrete payload the host
//! serializes.

use crate::styles_fs;

pub use crate::styles_fs::StyleOption;

/// Every selectable style — the file-backed catalog, sorted by label.
pub fn style_options() -> Vec<StyleOption> {
    styles_fs::style_options(styles_fs::TASKBAR_DIR)
}

/// Whether `id` is selectable: `""` (Windows Default) or a style file in
/// either layer.
pub fn is_style_id(id: &str) -> bool {
    id.is_empty() || styles_fs::style_exists(styles_fs::TASKBAR_DIR, id)
}

/// The theme's full converted payload — target styles, style constants,
/// theme-resource variables, and `Squircle`'s feature-gated variant.
pub fn theme(id: &str) -> Option<serde_json::Value> {
    styles_fs::load_theme(styles_fs::TASKBAR_DIR, id)
}

/// The OS feature id gating a theme's auto-variant (`Squircle` -> its
/// WeatherOnTheRight payload when `RtlQueryFeatureConfiguration` reports it
/// enabled). `None` for themes without a variant.
pub fn variant_feature(id: &str) -> Option<u32> {
    theme(id)?["variantFeature"].as_u64().map(|v| v as u32)
}

/// Serializes the theme into the line wire-format the injected
/// `coredesk_tap.dll` parses inside `explorer.exe` (the host transports it
/// with `WM_COPYDATA`). `use_variant` picks the `weatherVariant` payload when
/// present (the host resolves `variant_feature` via
/// `RtlQueryFeatureConfiguration`):
///
/// ```text
/// CDES-TBS1
/// T <target>
/// S <style>
/// ...
/// R <key>\t<value>       (theme resource variables)
/// E
/// ```
///
/// `$constant` substitution is done here (longest-name-first, the mod's own
/// ordering), so the DLL only ever sees expanded style strings.
pub fn theme_wire(id: &str, use_variant: bool) -> Option<String> {
    Some(wire_from_payload(&theme(id)?, use_variant))
}

/// The wire serializer against an already-loaded payload — kept separate
/// from file resolution so unit tests need no filesystem.
fn wire_from_payload(theme: &serde_json::Value, use_variant: bool) -> String {
    let theme = if use_variant && theme["weatherVariant"].is_object() {
        &theme["weatherVariant"]
    } else {
        theme
    };
    let mut out = String::with_capacity(64 * 1024);
    out.push_str("CDES-TBS1\n");

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

    if let Some(styles) = theme["targetStyles"].as_array() {
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

    /// A minimal `TranslucentTaskbar`-shaped payload exercising constants,
    /// `//` comments and a `Tag=>` bind.
    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "styleConstants": [
                "CommonBgBrush=<WindhawkBlur BlurAmount=\"18\" TintColor=\"#25323232\"/>",
                "// inline comments are allowed between constants"
            ],
            "targetStyles": [
                {
                    "target": "Taskbar.TaskbarFrame > Grid#RootGrid > Taskbar.TaskbarBackground > Grid > Rectangle#BackgroundFill",
                    "styles": ["Fill:=$CommonBgBrush"]
                },
                {
                    "target": "Taskbar.TaskListButton",
                    "styles": ["Tag=>taskbarDock", "Margin={{taskbarDock==`vertical`?skip():0}}"]
                },
                { "target": "// commented out", "styles": ["Margin=0"] }
            ]
        })
    }

    #[test]
    fn theme_wire_serializes_rules_and_resources() {
        let wire = wire_from_payload(&fixture(), false);
        assert!(wire.starts_with("CDES-TBS1\n"));
        assert!(wire.ends_with("E\n"));
        // No layout override or classic variant on the taskbar wire.
        assert!(!wire.contains("\nL "));
        assert!(!wire.contains("\nC\n"));
        assert!(wire.contains(
            "T Taskbar.TaskbarFrame > Grid#RootGrid > Taskbar.TaskbarBackground > Grid > Rectangle#BackgroundFill\n"
        ));
        // `$CommonBgBrush` must be expanded before transport.
        assert!(!wire.contains("$CommonBgBrush"));
        assert!(wire.contains("S Fill:=<WindhawkBlur"));
        // Dynamic expressions pass through untouched.
        assert!(wire.contains("S Margin={{taskbarDock==`vertical`?skip():0}}\n"));
    }

    #[test]
    fn weather_variant_swaps_the_whole_payload() {
        let theme = serde_json::json!({
            "targetStyles": [{"target": "A", "styles": ["X=1"]}],
            "variantFeature": 48660958,
            "weatherVariant": {"targetStyles": [{"target": "B", "styles": ["X=2"]}]},
        });
        let base = wire_from_payload(&theme, false);
        let variant = wire_from_payload(&theme, true);
        assert_ne!(base, variant);
        assert!(base.contains("T A\n"));
        assert!(variant.contains("T B\n"));
    }

    #[test]
    fn shipped_styles_serialize() {
        // End-to-end against the repo's shipped set when it is present
        // (also covered by styles_fs::tests::shipped_styles_all_parse).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../styles");
        if !root.is_dir() {
            return;
        }
        let doc =
            crate::styles_fs::load_in(&root, None, styles_fs::TASKBAR_DIR, "TranslucentTaskbar")
                .expect("shipped TranslucentTaskbar");
        let wire = wire_from_payload(&doc, false);
        assert!(wire.starts_with("CDES-TBS1\n"));
        assert!(wire.contains("T Taskbar.TaskbarFrame"));
        let squircle =
            crate::styles_fs::load_in(&root, None, styles_fs::TASKBAR_DIR, "Squircle").unwrap();
        assert_eq!(squircle["variantFeature"].as_u64(), Some(48660958));
    }
}
