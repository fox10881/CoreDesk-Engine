//! Filesystem-backed style catalogs — the data-driven modding layer.
//!
//! Six fixed category directories ([`CATEGORY_DIRS`]) hold one file per
//! style: `<id>.json` for the three shell surfaces and the live-effect
//! presets, `<id>.html` for the two widget design catalogs.
//!
//! Each is resolved across two layers: the factory set shipped with the app
//! at `<exe dir>\Styles\<category>\` and the user's editable set at
//! `<config dir>\Styles\<category>\` (registered once by
//! [`set_user_styles_root`] when the config directory is known). A user file
//! *shadows* the factory file of the same id — deleting the override
//! restores the shipped style, and reinstalling/resetting can never destroy
//! a modder's files.
//!
//! The file stem is the catalog id — the exact string `settings.*_style` and
//! `.cde` archives carry. Nothing here watches the filesystem: files are
//! read when a dropdown enumerates or a style applies; the delivered wire
//! stays cached in the host as before. The injected `coredesk_tap.dll`
//! never sees a file — the host parses the JSON and serializes the `CDES-*`
//! wire it already understands.
//!
//! A style file is the theme payload plus metadata:
//!
//! ```json
//! {
//!   "id": "Luminosity_variant_Dock",
//!   "label": "Luminosity — Dock",
//!   "author": "CoreDesk Engine",
//!   "version": 1,
//!   "styleConstants": ["..."],
//!   "targetStyles": [{"target": "...", "styles": ["..."]}],
//!   "themeResourceVariables": ["..."]
//! }
//! ```
//!
//! (`startMenuLayout`/`classicVariant`/`webViewTargetStyles` for the Start
//! Menu, `variantFeature`/`weatherVariant` for the taskbar's Squircle —
//! whichever fields the surface's payload ships.)

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// `Styles\` category directory names — the same names the installers
/// deploy under `{app}\Styles\` and the user's `Styles\` override root.
pub const TASKBAR_DIR: &str = "Taskbar style";
pub const START_MENU_DIR: &str = "Start Menu style";
pub const NOTIFICATION_CENTER_DIR: &str = "Notification Center style";
/// Self-contained WebView2 design documents — one `<id>.html` per clock
/// face / audio-visualizer renderer. The host `NavigateToString`s the
/// resolved file after swapping its `<!-- COREDESK_LOCALE -->` /
/// `<script src="i18n.js">` markers for the live payload.
pub const CLOCK_WIDGET_DIR: &str = "Clock Widget";
pub const VISUALIZER_WIDGET_DIR: &str = "Visualizer Widget";
/// Live-desktop-effect presets — one `<id>.json` per entry
/// (`{base, label, params}` composing a compiled `Effect` variant).
pub const LIVE_EFFECTS_DIR: &str = "Live Effects";

/// All six category directories in canonical order — the layout the
/// installers ship under `{app}\Styles\` and the scaffold the user layer
/// gets so an empty config still shows modders where files land.
pub const CATEGORY_DIRS: [&str; 6] = [
    TASKBAR_DIR,
    START_MENU_DIR,
    NOTIFICATION_CENTER_DIR,
    CLOCK_WIDGET_DIR,
    VISUALIZER_WIDGET_DIR,
    LIVE_EFFECTS_DIR,
];

/// A hand-edited style bigger than this is a mistake (the shipped set tops
/// out near 65 KB); refuse rather than stall the UI thread parsing it.
const MAX_STYLE_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// A dropdown entry: the style id (the file stem) plus the display label
/// from the file's `label` field, falling back to the id. `custom` marks
/// entries resolved from the user's editable layer.
#[derive(Clone, Debug)]
pub struct StyleOption {
    pub id: String,
    pub label: String,
    pub custom: bool,
}

/// Whether `id` is a legal style id: non-empty, NTFS-safe, no path
/// separators (id = file stem, and it arrives from settings/`.cde` input —
/// this is also the path-traversal guard for file resolution). This is the
/// only check [`crate::model`] validation may run without touching the
/// filesystem: a missing file must not clear the setting — it resolves to
/// Windows Default until the file comes back.
pub fn is_valid_style_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '&' | '.' | '-'))
}

/// `<exe dir>\Styles` — the shipped factory set. `COREDESK_STYLES_DIR`
/// overrides it (dev runs out of `target\`, fixture dirs in tests).
pub fn factory_styles_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("COREDESK_STYLES_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("Styles")))
        .unwrap_or_else(|| PathBuf::from("Styles"))
}

static USER_ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Points the user layer at `<config dir>\Styles` — called once when the
/// app's config directory is resolved. Portable installs pass their
/// portable config dir so mods travel with the app folder; the MSIX
/// edition's package-virtualized `%APPDATA%` is writable too.
pub fn set_user_styles_root(dir: PathBuf) {
    *USER_ROOT.write().unwrap_or_else(|p| p.into_inner()) = Some(dir);
}

/// The registered user layer root, if any.
pub fn user_styles_root() -> Option<PathBuf> {
    USER_ROOT.read().unwrap_or_else(|p| p.into_inner()).clone()
}

/// `MODDING_GUIDE.txt` — dropped at the user layer's root so the folder
/// explains itself the moment a modder opens it. Refreshed on every boot
/// when the shipped text drifts (new categories, new rules), so upgrades
/// carry the current contract.
const MODDING_GUIDE: &str = "\
CoreDesk Engine — Styles Folder (Modding Guide)
================================================

This folder is YOUR layer of the style catalog. It mirrors the factory
set shipped at <install>\\Styles — a file dropped here appears in the
matching Settings dropdown automatically. No restart, no registration.

Folders
-------
The engine creates all six for you — empty is normal, they mark where
each kind of file belongs.

  Taskbar style              Windows 11 taskbar visuals — one .json per style
  Start Menu style           Start menu visuals — one .json per style
  Notification Center style  Notification center visuals — one .json per style
  Clock Widget               Desktop clock designs — one .html per design
  Visualizer Widget          Audio visualizer designs — one .html per design
  Live Effects               Direct2D overlay presets — one .json per effect

File name = id
--------------
The file's name without its extension is what the engine saves, what .cde
themes reference, and what the dropdown lists. Name files like the
dropdown shows them (\"Orbital Geometry.html\", \"Neon Rain.json\").
Widget files may carry <!-- cde-meta {\"id\",\"label\"} --> at the top —
\"label\" overrides the dropdown text; without it the file name is shown.

Shadowing
---------
A file here OVERRIDES the factory file with the same name. Drop
\"Cyber Glass Flip.html\" into Clock Widget\\ and your version replaces
the stock design — delete it and the factory original returns.
Your files are never touched by installs or factory resets.

Formats
-------
- Shell styles (.json): the Windhawk-style theme document
  {\"id\",\"label\",\"targetStyles\",\"styleConstants\"}. Copy a factory
  file from <install>\\Styles and edit it — the schema is unchanged.
- Live Effects (.json): a preset over a compiled effect —
  {\"id\",\"label\",\"base\",\"params\"}. \"base\" must name a built-in
  effect (rain, snow, fireflies, matrix, …); \"params\" accepts
  particleCount, speed, wind, gravity, primaryColor, glowRadius.
- Widgets (.html): one self-contained page per design — inline your CSS,
  JS and assets (data URIs; there is no base URL for relative paths).
  After loading, post {\"type\":\"clockReady\"} or {\"type\":\"vizReady\"}
  so the host arms it; the host then drives the page with the design's
  messages (clockStyle/clockTheme/clockLocale, vizStyle/vizTheme and the
  33 ms vizSpectrum audio feed). A page may only post its own ready
  message — other channels are ignored.

Failure behavior
----------------
- A broken or unparseable .json style falls back to Windows Default when
  applied — a mistake can't wedge the shell.
- A missing or broken widget file keeps the current design; armed at a
  cold start it simply shows nothing until fixed.
- No file watching: dropdowns refresh when Settings opens or state is
  pushed; a file is read once when applied.
- Edit copies in THIS folder only — the factory set under <install>\\Styles
  is wiped by reinstalls, upgrades and factory resets.

.cde themes
-----------
A .cde theme stores only ids — never file contents. A theme referencing
\"Neon Rain\" needs \"Neon Rain.json\" present on the target machine, or
that slot silently falls back to the default. One exception: custom
files in THIS folder (and the relative image assets they reference)
are bundled inside the archive on export and unpacked back into this
folder when the theme is applied — factory styles are not bundled
since every install already ships them.
";

/// Scaffolds the user layer so the folder explains itself the moment a
/// modder opens it: every [`CATEGORY_DIRS`] subdirectory (empty dirs are
/// fine — they only mark where files land) plus [`MODDING_GUIDE`] at the
/// root. The guide rewrites only when the on-disk text differs — a user
/// edit holds until the shipped guide itself changes.
pub fn ensure_modding_guide() {
    let Some(root) = user_styles_root() else {
        return;
    };
    for category in CATEGORY_DIRS {
        let _ = std::fs::create_dir_all(root.join(category));
    }
    let path = root.join("MODDING_GUIDE.txt");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(MODDING_GUIDE) {
        let _ = std::fs::write(&path, MODDING_GUIDE);
    }
}

/// Every selectable style in `category`, sorted by label: the union of both
/// layers, user files shadowing factory ids. Files that don't parse
/// (mid-save, typos) are skipped — a broken file stays invisible rather
/// than selectable-and-dead; a broken *user* file also masks its factory
/// twin, so a deleted factory can't resurrect under an overriding stem.
pub fn style_options(category: &str) -> Vec<StyleOption> {
    enumerate_in(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
    )
}

/// [`style_options`] against explicit roots — the unit-testable core.
pub fn enumerate_in(factory: &Path, user: Option<&Path>, category: &str) -> Vec<StyleOption> {
    let mut by_id: BTreeMap<String, StyleOption> = BTreeMap::new();
    collect_dir(&factory.join(category), false, &mut by_id);
    if let Some(user) = user {
        collect_dir(&user.join(category), true, &mut by_id);
    }
    let mut opts: Vec<_> = by_id.into_values().collect();
    opts.sort_by_key(|a| a.label.to_lowercase());
    opts
}

fn collect_dir(dir: &Path, custom: bool, out: &mut BTreeMap<String, StyleOption>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_valid_style_id(id) {
            tracing::warn!(file = %path.display(), "style file has an invalid id — skipped");
            continue;
        }
        match read_theme(&path) {
            Some(doc) => {
                let label = doc["label"]
                    .as_str()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .unwrap_or(id)
                    .to_string();
                out.insert(
                    id.to_string(),
                    StyleOption {
                        id: id.to_string(),
                        label,
                        custom,
                    },
                );
            }
            None => {
                tracing::warn!(file = %path.display(), "style file failed to parse — skipped");
                if custom {
                    out.remove(id);
                }
            }
        }
    }
}

/// Whether `id` names a style file in either layer — the `is_style_id` gate
/// behind the dropdown setter. Cheap existence check; apply re-parses.
pub fn style_exists(category: &str, id: &str) -> bool {
    exists_in(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
        id,
    )
}

/// [`style_exists`] against explicit roots.
pub fn exists_in(factory: &Path, user: Option<&Path>, category: &str, id: &str) -> bool {
    is_valid_style_id(id) && resolve_path(factory, user, category, id).is_some()
}

/// The theme payload (`targetStyles`, `styleConstants`, …) for `id` — the
/// user layer first when its file exists. `None` when the id is absent or
/// the resolved file doesn't parse; callers treat that like an unknown id
/// (Windows Default at apply time).
pub fn load_theme(category: &str, id: &str) -> Option<Value> {
    load_in(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
        id,
    )
}

/// [`load_theme`] against explicit roots.
pub fn load_in(factory: &Path, user: Option<&Path>, category: &str, id: &str) -> Option<Value> {
    let path = resolve_path(factory, user, category, id)?;
    let mut theme = read_theme(&path)?;
    // Relative `ImageSource`/`Source` attributes resolve against the style
    // file's own directory — a mod ships `GTA VI.json` + `bg.jpg` side by
    // side and references it as `ImageSource="bg.jpg"`, keeping the file
    // portable (no absolute paths inside the document).
    if let Some(dir) = path.parent() {
        resolve_relative_uris(&mut theme, dir);
    }
    Some(theme)
}

/// Walks every string in the payload rewriting relative image URIs —
/// constants carry `:=` markup too, so the whole tree is scanned rather
/// than just `targetStyles`.
fn resolve_relative_uris(value: &mut Value, dir: &Path) {
    match value {
        Value::String(s) => *s = rewrite_relative_uris_in_text(s, dir),
        Value::Array(arr) => arr.iter_mut().for_each(|v| resolve_relative_uris(v, dir)),
        Value::Object(map) => map.values_mut().for_each(|v| resolve_relative_uris(v, dir)),
        _ => {}
    }
}

/// Rewrites `ImageSource="x.png"` / `Source="x.png"` attribute values that
/// carry no URI scheme to absolute `file:///` URIs under `dir`. Values with
/// a scheme (`file:///`, `https://`, `data:`, `ms-appx:`) pass through.
fn rewrite_relative_uris_in_text(text: &str, dir: &Path) -> String {
    if !text.contains("=\"") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    while let Some(eq) = rest.find("=\"") {
        // Attribute name = trailing identifier immediately before `=`
        // (`ImageSource=` keeps `Image` in the run — `Source=` alone only
        // matches an exact `Source` attribute, not e.g. `PropertySource=`).
        let name_start = rest[..eq]
            .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map(|p| p + 1)
            .unwrap_or(0);
        let name = &rest[name_start..eq];
        let val_start = eq + 2;
        let Some(close) = rest[val_start..].find('"') else {
            break;
        };
        let value = &rest[val_start..val_start + close];
        out.push_str(&rest[..val_start]);
        if matches!(name, "ImageSource" | "Source") && is_relative_uri(value) {
            out.push_str(&file_uri(dir, value));
        } else {
            out.push_str(value);
        }
        out.push('"');
        rest = &rest[val_start + close + 1..];
    }
    out.push_str(rest);
    out
}

/// A URI reference with no scheme and no traversal components — a bare
/// filename or clean relative subpath the style intends to resolve beside
/// its own file. Rejects `..`, `.`, drive prefixes, and rooted paths so a
/// style cannot reference or bundle files outside its own directory.
fn is_relative_uri(value: &str) -> bool {
    if value.is_empty() || value.starts_with('#') || value.starts_with(['\\', '/']) {
        return false;
    }
    if let Some(colon) = value.find(':')
        && value[..colon]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return false;
    }
    let p = Path::new(value);
    !value
        .split(['/', '\\'])
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// `dir` + a relative name → `file:///C:/…/Start%20Menu%20style/bg.jpg`.
/// UTF-8 outside the unreserved set is percent-encoded per-segment.
fn file_uri(dir: &Path, name: &str) -> String {
    use std::fmt::Write;
    let path = dir.join(name).to_string_lossy().replace('\\', "/");
    let mut uri = String::with_capacity(path.len() + 8);
    uri.push_str("file:///");
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(b as char)
            }
            _ => {
                let _ = write!(uri, "%{b:02X}");
            }
        }
    }
    uri
}

/// User layer first (an existing user file wins — even unreadable, so a
/// broken override can't silently behave like the factory theme), then the
/// shipped file.
fn resolve_path(factory: &Path, user: Option<&Path>, category: &str, id: &str) -> Option<PathBuf> {
    resolve_path_ext(factory, user, category, id, "json")
}

/// The resolved `.json` path for `id` in `category` across both layers —
/// the export side needs the file itself, not just its parsed payload.
pub fn resolve_file(category: &str, id: &str) -> Option<PathBuf> {
    resolve_path(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
        id,
    )
}

/// Same for `.html` widget documents.
pub fn resolve_widget_file(category: &str, id: &str) -> Option<PathBuf> {
    resolve_path_ext(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
        id,
        "html",
    )
}

/// Whether `path` lives under the registered user styles root — the split
/// that decides whether a `.cde` export must bundle a style file (factory
/// files already ship with the app on every machine).
pub fn is_user_layer_path(path: &Path) -> bool {
    user_styles_root().is_some_and(|root| path.starts_with(&root))
}

/// Files a style document references through relative `ImageSource=` /
/// `Source=` attributes, resolved to real files beside it — the assets a
/// `.cde` archive must carry so the style travels intact.
pub fn referenced_assets(style_path: &Path) -> Vec<PathBuf> {
    let Some(text) = read_text(style_path) else {
        return Vec::new();
    };
    let Some(dir) = style_path.parent() else {
        return Vec::new();
    };
    // Attributes live inside JSON string values — parse first so `\"` escapes
    // arrive as real quotes (matching `resolve_relative_uris`), then fall
    // back to scanning raw text for non-JSON mod files.
    let uris = match serde_json::from_str::<Value>(&text) {
        Ok(doc) => {
            let mut uris = Vec::new();
            collect_relative_uris_in_value(&doc, &mut uris);
            uris
        }
        Err(_) => collect_relative_attr_uris(&text),
    };
    let mut out = Vec::new();
    for value in uris {
        if !is_relative_uri(&value) {
            continue;
        }
        let path = dir.join(&value);
        if path.starts_with(dir) && path.is_file() && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// `collect_relative_attr_uris` over every string in a parsed document.
fn collect_relative_uris_in_value(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.extend(collect_relative_attr_uris(s)),
        Value::Array(arr) => arr
            .iter()
            .for_each(|v| collect_relative_uris_in_value(v, out)),
        Value::Object(map) => map
            .values()
            .for_each(|v| collect_relative_uris_in_value(v, out)),
        _ => {}
    }
}

/// Every scheme-less `ImageSource="x"`/`Source="x"` attribute value in
/// `text` — the inverse of [`rewrite_relative_uris_in_text`], collecting
/// rather than rewriting.
fn collect_relative_attr_uris(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(eq) = rest.find("=\"") {
        let name_start = rest[..eq]
            .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map(|p| p + 1)
            .unwrap_or(0);
        let name = &rest[name_start..eq];
        let val_start = eq + 2;
        let Some(close) = rest[val_start..].find('"') else {
            break;
        };
        let value = &rest[val_start..val_start + close];
        if matches!(name, "ImageSource" | "Source")
            && is_relative_uri(value)
            && !out.iter().any(|v| v == value)
        {
            out.push(value.to_string());
        }
        rest = &rest[val_start + close + 1..];
    }
    out
}

/// [`resolve_path`] parameterized by file extension — `.json` style files
/// and `.html` widget documents share the resolution/shadowing rules.
fn resolve_path_ext(
    factory: &Path,
    user: Option<&Path>,
    category: &str,
    id: &str,
    ext: &str,
) -> Option<PathBuf> {
    if !is_valid_style_id(id) {
        return None;
    }
    let name = format!("{id}.{ext}");
    if let Some(user) = user {
        let path = user.join(category).join(&name);
        if path.is_file() {
            return Some(path);
        }
    }
    let path = factory.join(category).join(name);
    path.is_file().then_some(path)
}

/// Reads one file as UTF-8 text: size-capped, BOM tolerated (Notepad writes
/// it). `None` on any failure — callers warn or treat the file as absent.
fn read_text(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_STYLE_FILE_BYTES {
        tracing::warn!(
            file = %path.display(),
            max = MAX_STYLE_FILE_BYTES,
            "style file exceeds the size cap — skipped"
        );
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    Some(text.strip_prefix('\u{feff}').unwrap_or(text).to_string())
}

/// The WebView2 design document (`<id>.html`) for `id` in `category`
/// ([`CLOCK_WIDGET_DIR`]/[`VISUALIZER_WIDGET_DIR`]) — user layer first.
/// The host navigates to the returned text after injecting the locale
/// payload markers; `None` when the id is absent or the file is unreadable.
pub fn load_widget_document(category: &str, id: &str) -> Option<String> {
    read_text(&resolve_path_ext(
        &factory_styles_root(),
        user_styles_root().as_deref(),
        category,
        id,
        "html",
    )?)
}

/// Every widget design document in `category`, same option contract as
/// [`style_options`]: id = file stem, `custom` marks user-layer files,
/// label comes from the document's `<!-- cde-meta {"label": …} -->` header
/// comment, falling back to the stem. Unparseable HTML is not rejected —
/// the meta scan is best-effort so a hand-edited file always stays
/// selectable (a broken document just renders nothing when applied).
pub fn widget_options(category: &str) -> Vec<StyleOption> {
    let mut by_id: BTreeMap<String, StyleOption> = BTreeMap::new();
    collect_widget_dir(&factory_styles_root().join(category), false, &mut by_id);
    if let Some(user) = user_styles_root() {
        collect_widget_dir(&user.join(category), true, &mut by_id);
    }
    let mut opts: Vec<_> = by_id.into_values().collect();
    opts.sort_by_key(|a| a.label.to_lowercase());
    opts
}

fn collect_widget_dir(dir: &Path, custom: bool, out: &mut BTreeMap<String, StyleOption>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("html"))
        {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_valid_style_id(id) {
            tracing::warn!(file = %path.display(), "widget file has an invalid id — skipped");
            continue;
        }
        let label = read_text(&path)
            .and_then(|doc| widget_meta_label(&doc))
            .unwrap_or_else(|| id.to_string());
        out.insert(
            id.to_string(),
            StyleOption {
                id: id.to_string(),
                label,
                custom,
            },
        );
    }
}

/// `<!-- cde-meta {"id":"Zen Minimal Ribbon","label":"Zen Minimal Ribbon",…} -->` — the optional
/// header comment carrying a design's display metadata. Scanned from the
/// document head only (first 8 KB) so giant modded files still enumerate
/// cheaply; the JSON is parsed loosely — `label` is all we consume.
fn widget_meta_label(doc: &str) -> Option<String> {
    let head = &doc[..doc.len().min(8192)];
    let start = head.find("<!--")?;
    let rest = &head[start + 4..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("cde-meta")?;
    let end = rest.find("-->")?;
    let meta: Value = serde_json::from_str(rest[..end].trim()).ok()?;
    meta["label"]
        .as_str()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
}

/// The option list as a JSON value for the settings state payload —
/// `{id, label, custom}` per file plus the synthesized `none` entry the
/// widget dropdowns lead with (label = the source-language key the UI
/// localizes through `t()`).
pub fn widget_options_json(category: &str, none_label: &str) -> serde_json::Value {
    let mut arr = vec![serde_json::json!({
        "id": "none",
        "label": none_label,
        "custom": false,
    })];
    arr.extend(widget_options(category).into_iter().map(|o| {
        serde_json::json!({
            "id": o.id,
            "label": o.label,
            "custom": o.custom,
        })
    }));
    serde_json::Value::Array(arr)
}

/// Reads + parses one style file: object JSON, UTF-8, BOM tolerated
/// (Notepad writes it), size-capped. `None` on any failure — callers warn
/// or treat the file as absent.
fn read_theme(path: &Path) -> Option<Value> {
    let doc: Value = serde_json::from_str(&read_text(path)?).ok()?;
    doc.is_object().then_some(doc)
}

/// `ApplyStyleConstants` from the Windhawk mods: replace each `$name` token
/// with its constant's value; unknown `$x` survives literally. Shared by
/// the three surface serializers.
pub(crate) fn expand_constants(input: &str, constants: &[(String, String)]) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let rest = &input[i + 1..];
            if let Some((name, value)) =
                constants.iter().find(|(n, _)| rest.starts_with(n.as_str()))
            {
                out.push_str(value);
                i += 1 + name.len();
                continue;
            }
        }
        let ch_len = input[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&input[i..i + ch_len]);
        i += ch_len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), body).unwrap();
    }

    fn theme_doc(label: &str) -> String {
        format!(
            r#"{{"id":"x","label":"{label}","targetStyles":[{{"target":"T","styles":["Margin=0"]}}]}}"#
        )
    }

    #[test]
    fn ids_validate_by_charset_not_existence() {
        assert!(is_valid_style_id("Luminosity_variant_Dock"));
        assert!(is_valid_style_id("Oversimplified&Accentuated"));
        assert!(is_valid_style_id("Command Center"));
        assert!(is_valid_style_id("OS26 Tahoe Glass"));
        assert!(!is_valid_style_id(""));
        assert!(!is_valid_style_id("../escape"));
        assert!(!is_valid_style_id("a/b"));
        assert!(!is_valid_style_id("a\\b"));
        assert!(!is_valid_style_id("a:b"));
        assert!(!is_valid_style_id("a<b"));
        assert!(!is_valid_style_id("a?b"));
        assert!(!is_valid_style_id("a|b"));
        assert!(!is_valid_style_id("a*b"));
        assert!(!is_valid_style_id(&"x".repeat(200)));
    }

    #[test]
    fn user_layer_shadows_factory_by_stem() {
        let base = std::env::temp_dir().join(format!("stylesfs-{}", std::process::id()));
        let factory = base.join("factory");
        let user = base.join("user");
        write(
            &factory.join(TASKBAR_DIR),
            "One.json",
            &theme_doc("Factory One"),
        );
        write(
            &factory.join(TASKBAR_DIR),
            "Two.json",
            &theme_doc("Factory Two"),
        );
        write(&user.join(TASKBAR_DIR), "One.json", &theme_doc("My One"));

        let opts = enumerate_in(&factory, Some(&user), TASKBAR_DIR);
        assert_eq!(opts.len(), 2);
        let one = opts.iter().find(|o| o.id == "One").unwrap();
        assert_eq!(one.label, "My One");
        assert!(one.custom);
        let two = opts.iter().find(|o| o.id == "Two").unwrap();
        assert!(!two.custom);

        let doc = load_in(&factory, Some(&user), TASKBAR_DIR, "One").unwrap();
        assert_eq!(doc["label"], "My One");
        let doc = load_in(&factory, Some(&user), TASKBAR_DIR, "Two").unwrap();
        assert_eq!(doc["label"], "Factory Two");
        assert!(exists_in(&factory, Some(&user), TASKBAR_DIR, "One"));
        assert!(!exists_in(&factory, Some(&user), TASKBAR_DIR, "Missing"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn broken_files_hide_and_broken_user_file_masks_factory() {
        let base = std::env::temp_dir().join(format!("stylesfs-bad-{}", std::process::id()));
        let factory = base.join("factory");
        let user = base.join("user");
        write(&factory.join(TASKBAR_DIR), "Ok.json", &theme_doc("Ok"));
        write(&factory.join(TASKBAR_DIR), "Broken.json", "{not json");
        write(&user.join(TASKBAR_DIR), "Ok.json", "{still broken");

        let opts = enumerate_in(&factory, Some(&user), TASKBAR_DIR);
        assert!(
            opts.is_empty(),
            "broken factory + broken override hide both"
        );
        assert!(load_in(&factory, Some(&user), TASKBAR_DIR, "Ok").is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn relative_image_uris_resolve_against_the_style_dir() {
        let base = std::env::temp_dir().join(format!("stylesfs-uri-{}", std::process::id()));
        let factory = base.join("factory");
        write(
            &factory.join(TASKBAR_DIR),
            "Img.json",
            &serde_json::json!({
                "styleConstants": [
                    "Wall=<ImageBrush ImageSource=\"bg.jpg\" Stretch=\"UniformToFill\"/>"
                ],
                "targetStyles": [
                    { "target": "Border", "styles": [
                        "Background:=$Wall",
                        "Background:=<ImageBrush ImageSource=\"icon.png\"/>",
                        "Fill:=<ImageBrush ImageSource=\"file:///D:/abs.png\"/>",
                        "Foreground:=<ImageBrush ImageSource=\"data:image/png;base64,AAAA\"/>"
                    ] }
                ]
            })
            .to_string(),
        );
        let doc = load_in(&factory, None, TASKBAR_DIR, "Img").unwrap();
        let txt = doc.to_string();
        let dir = factory
            .join(TASKBAR_DIR)
            .to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "%20");
        let dir = dir.trim_start_matches("//?/");
        assert!(txt.contains(&format!("file:///{dir}/bg.jpg")), "{txt}");
        assert!(txt.contains(&format!("file:///{dir}/icon.png")), "{txt}");
        assert!(txt.contains("file:///D:/abs.png"), "{txt}");
        assert!(txt.contains("data:image/png;base64,AAAA"), "{txt}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn referenced_assets_collects_only_existing_relative_files() {
        let base = std::env::temp_dir().join(format!("stylesfs-refs-{}", std::process::id()));
        let dir = base.join(TASKBAR_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bg.jpg"), b"jpeg-bytes").unwrap();
        fs::write(dir.join("icon.png"), b"png").unwrap();
        let style = dir.join("Mod.json");
        let outside = base.join("secret.png");
        fs::write(&outside, b"secret").unwrap();
        fs::write(
            &style,
            r#"{"styleConstants":[
                "W=<ImageBrush ImageSource=\"bg.jpg\"/>",
                "I=<ImageBrush ImageSource=\"icon.png\"/>",
                "M=<ImageBrush ImageSource=\"missing.png\"/>",
                "T1=<ImageBrush ImageSource=\"../secret.png\"/>",
                "T2=<ImageBrush ImageSource=\"..\\secret.png\"/>",
                "A=<ImageBrush ImageSource=\"data:image/png;base64,AAAA\"/>",
                "B=<ImageBrush ImageSource=\"file:///D:/abs.png\"/>"
            ]}"#,
        )
        .unwrap();
        assert!(!is_relative_uri("../secret.png"));
        assert!(!is_relative_uri("..\\secret.png"));
        assert!(!is_relative_uri("./bg.jpg"));
        let mut names: Vec<_> = referenced_assets(&style)
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        assert_eq!(names, ["bg.jpg", "icon.png"]);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn label_falls_back_to_stem_and_bom_is_tolerated() {
        let base = std::env::temp_dir().join(format!("stylesfs-bom-{}", std::process::id()));
        let factory = base.join("factory");
        write(
            &factory.join(TASKBAR_DIR),
            "Plain.json",
            "{\"targetStyles\":[]}",
        );
        write(
            &factory.join(TASKBAR_DIR),
            "Bommed.json",
            "\u{feff}{\"label\":\"Has BOM\"}",
        );
        let opts = enumerate_in(&factory, None, TASKBAR_DIR);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "Has BOM");
        assert_eq!(opts[1].id, "Plain");
        assert_eq!(opts[1].label, "Plain");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn shipped_styles_all_parse() {
        // The repo's `styles/` tree is the shipped factory set — every file
        // must stay loadable. `CARGO_MANIFEST_DIR` is `crates/core`.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../styles");
        if !root.is_dir() {
            return; // packaged source trees may not carry it
        }
        for category in [
            TASKBAR_DIR,
            START_MENU_DIR,
            NOTIFICATION_CENTER_DIR,
            LIVE_EFFECTS_DIR,
        ] {
            let opts = enumerate_in(&root, None, category);
            assert!(
                !opts.is_empty(),
                "factory category {category} must ship styles"
            );
            for o in &opts {
                let id = &o.id;
                assert!(
                    load_in(&root, None, category, id).is_some(),
                    "{category}/{id} must load"
                );
            }
        }
        // Live-Effects presets must name a compiled `base` and carry a
        // parseable `params` object — a preset without either is a broken
        // factory file.
        let fx = enumerate_in(&root, None, LIVE_EFFECTS_DIR);
        for o in &fx {
            let doc = load_in(&root, None, LIVE_EFFECTS_DIR, &o.id).unwrap();
            let base = doc["base"].as_str().expect("preset needs a base");
            assert!(
                crate::EFFECT_IDS.contains(&base),
                "preset {} has unknown base {base}",
                o.id
            );
            assert!(
                doc["params"].is_object(),
                "preset {} needs a params object",
                o.id
            );
        }
    }

    #[test]
    fn shipped_widget_documents_enumerate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../styles");
        if !root.is_dir() {
            return;
        }
        for category in [CLOCK_WIDGET_DIR, VISUALIZER_WIDGET_DIR] {
            let mut by_id = BTreeMap::new();
            collect_widget_dir(&root.join(category), false, &mut by_id);
            assert!(
                !by_id.is_empty(),
                "factory category {category} must ship designs"
            );
            for (id, opt) in &by_id {
                // Every shipped design loads as text and carries a label
                // (cde-meta) distinct from its bare id.
                let path = root.join(category).join(format!("{id}.html"));
                let doc = read_text(&path).expect("{category}/{id} must read");
                assert!(doc.contains("<html"), "{category}/{id} is not a document");
                assert_eq!(
                    widget_meta_label(&doc).as_deref(),
                    Some(opt.label.as_str()),
                    "{category}/{id} label must come from its cde-meta"
                );
            }
        }
    }

    #[test]
    fn user_layer_scaffolds_all_categories_and_guide() {
        let base = std::env::temp_dir().join(format!("stylesfs-scaffold-{}", std::process::id()));
        let root = base.join("Styles");
        set_user_styles_root(root.clone());
        ensure_modding_guide();
        for category in CATEGORY_DIRS {
            assert!(
                root.join(category).is_dir(),
                "category {category} must be scaffolded"
            );
        }
        assert!(root.join("MODDING_GUIDE.txt").is_file());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn constants_expand_longest_first() {
        // The serializers sort constants longest-name-first before use —
        // mirror that order here (`$CommonBg` must not lose to `$Common`).
        let constants = vec![
            ("CommonBg".to_string(), "Y".to_string()),
            ("Common".to_string(), "X".to_string()),
        ];
        assert_eq!(
            expand_constants("$CommonBg/$Common/$Nope", &constants),
            "Y/X/$Nope"
        );
    }
}
