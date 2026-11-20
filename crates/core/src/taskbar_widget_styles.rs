//! Taskbar resource-widget style catalog — `{id, font, metrics, colors}`
//! descriptors as user-editable files under `Styles\Taskbar Widget\`
//! (two-layer resolution: [`crate::styles_fs`]). One `<id>.json` per
//! design, rendered by the native Direct2D taskbar overlay — a descriptor
//! can only describe, never execute, so the catalog needs no sandbox.
//!
//! `id` is the style file stem; `""`/`"none"` is Windows Default (widget
//! hidden — the off state the Settings toggle writes).

use crate::styles_fs;

pub use crate::styles_fs::StyleOption;

/// Every selectable style — the file-backed catalog, sorted by label.
pub fn style_options() -> Vec<StyleOption> {
    styles_fs::style_options(styles_fs::TASKBAR_WIDGET_DIR)
}

/// Whether `id` is selectable: `""`/`"none"` (hidden) or a style file in
/// either layer.
pub fn is_style_id(id: &str) -> bool {
    id.is_empty() || id == "none" || styles_fs::style_exists(styles_fs::TASKBAR_WIDGET_DIR, id)
}

/// The option list as a JSON value for the settings state payload —
/// 「无（关闭）」 first, then every resolvable file (`custom` marks
/// user-layer overrides).
pub fn style_options_json() -> serde_json::Value {
    let mut arr = vec![serde_json::json!({
        "id": "none",
        "label": "无（关闭）",
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

/// A metric slot the style can request, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Ram,
    Gpu,
    Net,
}

/// A resolved text color for the overlay: `None` = adaptive (the
/// backdrop-probe sample picks the contrast pair), `Some(rgb)` = the
/// style's authored `#RRGGBB`/`#AARRGGBB` (alpha channel ignored for
/// opaque text; kept for future translucent fills).
pub type StyleColor = Option<[u8; 3]>;

/// The strip's chrome behind the text. `Glass` draws the dark translucent
/// capsule + accent rim (the clock-widget look); `Plain` keeps the bare
/// text-on-taskbar strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TbWidgetChrome {
    #[default]
    Glass,
    Plain,
}

/// A parsed `.json` descriptor — every field carries the compiled default
/// a modder sees in the shipped file, so `None`-fields never change the
/// rendered output.
#[derive(Clone, Debug)]
pub struct TbWidgetStyle {
    pub font: String,
    /// DIP size at 96 DPI.
    pub font_size: f32,
    /// 100..=900 DirectWrite weight.
    pub font_weight: u32,
    pub metrics: Vec<Metric>,
    pub separator: String,
    /// Horizontal inset in DIPs.
    pub padding_x: f32,
    pub text_color: StyleColor,
    pub accent_color: StyleColor,
    pub shadow: bool,
    pub chrome: TbWidgetChrome,
}

impl Default for TbWidgetStyle {
    fn default() -> Self {
        Self {
            font: "Segoe UI".to_string(),
            font_size: 11.0,
            font_weight: 600,
            metrics: vec![Metric::Cpu, Metric::Ram],
            separator: "  ·  ".to_string(),
            padding_x: 8.0,
            text_color: None,
            accent_color: None,
            shadow: true,
            chrome: TbWidgetChrome::Glass,
        }
    }
}

/// `"#RRGGBB"`/`"#AARRGGBB"`/`"auto"` → [`StyleColor`]. Anything else
/// (including `"none"` typos) parses as `None` — adaptive — never a hard
/// failure on a hand-edited file.
fn parse_color(v: &serde_json::Value) -> StyleColor {
    let s = v.as_str()?;
    let s = s.trim().trim_start_matches('#');
    let rgb = match s.len() {
        6 => &s[0..6],
        8 => &s[2..8],
        _ => return None,
    };
    let Ok(r) = u8::from_str_radix(&rgb[0..2], 16) else {
        return None;
    };
    let Ok(g) = u8::from_str_radix(&rgb[2..4], 16) else {
        return None;
    };
    let Ok(b) = u8::from_str_radix(&rgb[4..6], 16) else {
        return None;
    };
    Some([r, g, b])
}

/// The parsed descriptor for `id` — `None` when the id resolves to no file
/// or the file doesn't parse (callers treat that like an unknown id: the
/// widget simply doesn't arm).
pub fn style(id: &str) -> Option<TbWidgetStyle> {
    let doc = styles_fs::load_theme(styles_fs::TASKBAR_WIDGET_DIR, id)?;
    let mut out = TbWidgetStyle::default();
    if let Some(f) = doc["font"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        out.font = f.to_string();
    }
    if let Some(n) = doc["fontSize"].as_f64() {
        out.font_size = (n as f32).clamp(6.0, 48.0);
    }
    if let Some(n) = doc["fontWeight"].as_u64() {
        out.font_weight = (n as u32).clamp(100, 900);
    }
    if let Some(arr) = doc["metrics"].as_array() {
        let metrics: Vec<Metric> = arr
            .iter()
            .filter_map(|v| match v.as_str()?.to_ascii_lowercase().as_str() {
                "cpu" => Some(Metric::Cpu),
                "ram" => Some(Metric::Ram),
                "gpu" => Some(Metric::Gpu),
                "net" => Some(Metric::Net),
                _ => None,
            })
            .collect();
        if !metrics.is_empty() {
            out.metrics = metrics;
        }
    }
    if let Some(s) = doc["separator"].as_str() {
        out.separator = s.to_string();
    }
    if let Some(n) = doc["paddingX"].as_f64() {
        out.padding_x = (n as f32).clamp(0.0, 64.0);
    }
    if doc["textColor"].is_string() {
        out.text_color = parse_color(&doc["textColor"]);
    }
    if doc["accentColor"].is_string() {
        out.accent_color = parse_color(&doc["accentColor"]);
    }
    if let Some(b) = doc["shadow"].as_bool() {
        out.shadow = b;
    }
    if let Some(s) = doc["chrome"].as_str() {
        // "none"/"flat"/"plain" opt back into the bare-text strip;
        // anything else (including absent) keeps the glass capsule.
        out.chrome = match s.trim().to_ascii_lowercase().as_str() {
            "none" | "flat" | "plain" | "off" => TbWidgetChrome::Plain,
            _ => TbWidgetChrome::Glass,
        };
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_color_variants() {
        assert_eq!(
            parse_color(&serde_json::json!("#FF8800")),
            Some([0xFF, 0x88, 0x00])
        );
        assert_eq!(
            parse_color(&serde_json::json!("#CCFF8800")),
            Some([0xFF, 0x88, 0x00])
        );
        assert_eq!(parse_color(&serde_json::json!("auto")), None);
        assert_eq!(parse_color(&serde_json::json!("#GGHHII")), None);
        assert_eq!(parse_color(&serde_json::json!(42)), None);
    }
}
