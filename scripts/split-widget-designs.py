#!/usr/bin/env python3
"""Split the monolithic widget galleries into per-design mod files.

`ui/clock.html` carries 25 designs (`.style-gN` CSS + a `BUILDS` JS entry
each) and `ui/visualizer.html` carries 10 canvas renderers — this script
emits one self-contained `styles/<Category>/<id>.html` per design, the
same shape a hand-written mod file takes:

- the page scaffold (head, widget root, i18n markers for the clock),
- a `<!-- cde-meta {"id","label",...} -->` header the dropdown reads,
- only that design's CSS (base rules + its block + its light-backdrop
  palette overrides),
- the shared runtime with a single-entry `BUILDS` table and the design
  pinned as `DESIGN_ID` — the file *is* the design, so a mod copy works
  under any filename/id.

Re-run after editing the source galleries. The emitted files are the
shipped payload (`{app}\\Styles\\…`); the monoliths stay as the authoring
source, like `styles_source/` does for the XAML catalogs.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
STYLES = ROOT / "styles"

CLOCK_DIR = STYLES / "Clock Widget"
VIZ_DIR = STYLES / "Visualizer Widget"
FX_DIR = STYLES / "Live Effects"

# Chinese source-language labels — the same keys the settings UI feeds
# t(); factory files carry them so pushed options localize for free.
CLOCK_LABELS = {
    "g1": "玻璃翻页", "g2": "霓虹时环", "g3": "瑞士粗体", "g4": "点阵数码",
    "g5": "流光胶囊", "g6": "奢华指针", "g7": "分割网格", "g8": "日月地平线",
    "g9": "立体堆叠", "g10": "禅意缎带", "d1": "极光霜面", "d2": "霓虹终端",
    "d3": "时间轨道", "d4": "衬线卡片", "d5": "柔和表盘", "d6": "全息面板",
    "d7": "日历页", "d8": "渐变块", "d9": "日期海报", "d10": "紧凑胶囊",
    "g11": "霓虹电路面板", "g12": "极简竖排", "g13": "复古翻牌",
    "g14": "几何轨道", "g15": "古典指针",
}
VIZ_LABELS = {
    "v1": "粒子星河", "v2": "弧光反应环", "v3": "频谱缎带", "v4": "霓虹地平线",
    "v5": "共鸣光球", "v6": "低音花绽", "v7": "律动天际线", "v8": "螺旋音序",
    "v9": "彗星音轨", "v10": "脉冲点阵",
}
EFFECT_LABELS = {
    "rain": "流体雨滴", "thunderstorm": "雷雨风暴", "snow": "飘落雪花",
    "fireflies": "萤火之光", "embers": "升腾余烬", "sakura": "樱吹雪",
    "mist": "缥缈薄雾", "aurora": "北极光", "starfield": "星空尘埃",
    "matrix": "数码矩阵", "ripples": "水波涟漪", "constellation": "星链网络",
    "magic_trail": "魔法霓虹轨迹", "shatter_clicks": "点击玻璃碎裂",
    "quantum_swarm": "量子磁力虫群", "cyber_grid_reveal": "赛博网格显影",
    "repulsion_dust": "物理斥力尘埃", "bokeh": "梦幻光斑", "leaves": "秋叶飘零",
    "warp": "曲速星河", "pulse": "声呐脉冲", "meteors": "流星雨",
    "le_rain": "共鸣雨幕（音频）", "le_grid": "声控霓虹地平线（音频）",
    "le_bloom": "环境呼吸光（音频）", "le_aurora": "频谱极光（音频）",
    "le_rings": "脉冲音环（音频）",
}


def meta(design_id: str, label: str) -> str:
    return (
        '<!-- cde-meta '
        + json.dumps(
            {"id": design_id, "label": label, "author": "CoreDesk Engine", "version": 1},
            ensure_ascii=False,
        )
        + " -->\n"
    )


def split_palette(palette: str, design_id: str):
    """Split the `body.light-backdrop` block into (shared, per-design) CSS.

    Shared = the transition rule + the bare `body.light-backdrop` var
    overrides + header comments; per-design = the design's accent-text
    selectors and every standalone rule mentioning its classes. Selectors
    belonging to *other* designs are dropped from the file entirely.
    """
    token = re.compile(r"(?<![0-9a-zA-Z])" + re.escape(design_id) + r"(?![0-9a-zA-Z])")
    shared, mine = [], []
    # Split into rules: selector text + `{body}` pairs (no nested braces).
    for sel, body in re.findall(r"([^{}]+)\{([^{}]*)\}", palette, re.S):
        sel_nocom = re.sub(r"/\*.*?\*/", "", sel, flags=re.S).strip()
        if not sel_nocom.startswith("body.light-backdrop") or sel_nocom == "body.light-backdrop":
            shared.append(sel + "{" + body + "}")
            continue
        # A `body.light-backdrop <sels> {…}` rule: keep only this design's
        # selectors; leading comments move to shared so the palette stays
        # documented in every file.
        for c in re.findall(r"/\*.*?\*/", sel, flags=re.S):
            shared.append("  " + c.strip() + "\n")
        sels = [s.strip() for s in re.split(r",\s*\n?\s*", sel_nocom)]
        keep = [s for s in sels if token.search(s)]
        if keep:
            mine.append(",\n  ".join(keep) + " {" + body + "}")
    return "".join(shared), "\n  ".join(mine)


def split_clock():
    text = (UI / "clock.html").read_text(encoding="utf-8")
    lines = text.split("\n")

    # --- style block boundaries -------------------------------------------
    style_open = next(i for i, l in enumerate(lines) if l.strip() == "<style>")
    gemini = next(i for i, l in enumerate(lines) if "Gemini g1" in l)
    palette_mark = next(
        i for i, l in enumerate(lines) if "Adaptive backdrop palette" in l
    )
    style_close = next(i for i, l in enumerate(lines) if l.strip() == "</style>")

    base_css = "\n".join(lines[style_open + 1 : gemini])

    # --- per-design CSS sections: `/* GN —` / `/* DN —` comments ----------
    sec_re = re.compile(r"/\* ([GD])(\d+) —")
    sections, cur_id, cur = {}, None, []
    for l in lines[gemini + 1 : palette_mark]:
        m = sec_re.search(l)
        if m:
            if cur_id:
                sections[cur_id] = "\n".join(cur).strip("\n")
            cur_id = (m.group(1) + m.group(2)).lower()
            cur = [l]
        elif cur_id:
            cur.append(l)
    if cur_id:
        sections[cur_id] = "\n".join(cur).strip("\n")

    palette = "\n".join(lines[palette_mark:style_close])

    # --- JS: prologue, BUILDS entries, runtime ----------------------------
    builds_open = next(i for i, l in enumerate(lines) if "const BUILDS = {" in l)
    prologue = "\n".join(lines[lines.index("<script>", style_close) + 1 : builds_open])
    builds_close = next(i for i, l in enumerate(lines) if l.strip() == "};" and i > builds_open)
    entry_re = re.compile(r"^    ([gd]\d+):")
    entries, cur_id, cur = {}, None, []
    for l in lines[builds_open + 1 : builds_close]:
        m = entry_re.match(l)
        if m:
            if cur_id:
                entries[cur_id] = "\n".join(cur).rstrip().rstrip(",").rstrip()
            cur_id, cur = m.group(1), [l]
        elif cur_id:
            cur.append(l)
    if cur_id:
        entries[cur_id] = "\n".join(cur).rstrip().rstrip(",").rstrip()

    runtime = "\n".join(lines[builds_close + 1 :])
    # Per-design file: the page *is* its design — `clockStyle` (re)builds
    # DESIGN_ID regardless of the pushed id so a mod copy under any file
    # name keeps working.
    runtime = runtime.replace(
        "if (m.t === 'clockStyle' && BUILDS[m.style]) {",
        "if (m.t === 'clockStyle') {",
    )
    runtime = runtime.replace("setDesign(m.style);", "setDesign(DESIGN_ID);")

    head = "\n".join(lines[:5])  # doctype … viewport meta
    locale_line = lines[5]  # <!-- COREDESK_LOCALE -->
    title_i18n = "\n".join(lines[6:9])  # <title> + i18n.js + <style>

    CLOCK_DIR.mkdir(parents=True, exist_ok=True)
    count = 0
    for did, label in CLOCK_LABELS.items():
        css_sec = sections.get(did)
        entry = entries.get(did)
        if not css_sec or not entry:
            print(f"  !! {did}: missing CSS or JS section — skipped", file=sys.stderr)
            continue
        pal_shared, pal_mine = split_palette(palette, did)
        doc = (
            head + "\n"
            + meta(did, label)
            + locale_line + "\n"
            + title_i18n + "\n"
            + base_css
            + "\n  " + css_sec.replace("\n", "\n  ").rstrip() + "\n"
            + pal_shared
            + ("\n  " + pal_mine.replace("\n", "\n  ").rstrip() + "\n" if pal_mine else "")
            + "</style>\n</head>\n<body>\n<div id=\"widget-root\"></div>\n<script>\n"
            + prologue
            + "\n  const DESIGN_ID = '" + did + "';\n"
            + "  const BUILDS = {\n" + entry + "\n  };\n"
            + runtime
        )
        (CLOCK_DIR / f"{did}.html").write_text(doc, encoding="utf-8")
        count += 1
    print(f"clock: {count} design files -> {CLOCK_DIR}")


def split_visualizer():
    text = (UI / "visualizer.html").read_text(encoding="utf-8")
    lines = text.split("\n")

    builds_open = next(i for i, l in enumerate(lines) if "const BUILDS = {" in l)
    scaffold = "\n".join(lines[: builds_open + 1])
    scaffold = scaffold.replace("style = 'v1'", "style = '@@ID@@'")

    entry_re = re.compile(r"^  (v\d+)\(\)")
    # The table's own `};` sits at column 0 — inner `    };` lines don't.
    builds_close = next(i for i, l in enumerate(lines) if l == "};" and i > builds_open)
    entries, cur_id, cur = {}, None, []
    for l in lines[builds_open + 1 : builds_close]:
        m = entry_re.match(l)
        if m:
            if cur_id:
                entries[cur_id] = "\n".join(cur).rstrip().rstrip(",").rstrip()
            cur_id, cur = m.group(1), [l]
        elif cur_id:
            cur.append(l)
    if cur_id:
        entries[cur_id] = "\n".join(cur).rstrip().rstrip(",").rstrip()

    tail = "\n".join(lines[builds_close + 1 :])
    # The file is its design — `vizStyle` rebuilds the pinned entry.
    tail = tail.replace(
        "else if (m.t === 'vizStyle' && BUILDS[m.style]) {\n      style = m.style;\n      R = BUILDS[style]();",
        "else if (m.t === 'vizStyle') {\n      R = BUILDS[style]();",
    )

    VIZ_DIR.mkdir(parents=True, exist_ok=True)
    count = 0
    for did, label in VIZ_LABELS.items():
        entry = entries.get(did)
        if not entry:
            print(f"  !! {did}: missing JS factory — skipped", file=sys.stderr)
            continue
        doc = (
            scaffold.replace("style = '@@ID@@'", f"style = '{did}'").replace(
                "</head>", meta(did, label) + "</head>", 1
            )
            + "\n  " + entry + "\n};\n"
            + tail
        )
        (VIZ_DIR / f"{did}.html").write_text(doc, encoding="utf-8")
        count += 1
    print(f"visualizer: {count} design files -> {VIZ_DIR}")


def emit_effect_presets():
    """Factory `Live Effects` presets — `{base, label, params}` per compiled
    effect, params mirroring the authored `Live_Effects_config` baseline."""
    baseline_path = ROOT / "target/debug/config/Live_Effects_config.json"
    baseline = {}
    if baseline_path.is_file():
        baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    FX_DIR.mkdir(parents=True, exist_ok=True)
    count = 0
    for did, label in EFFECT_LABELS.items():
        base = baseline.get(did, {})
        params = {
            k: base.get(k)
            for k in ("particleCount", "speed", "wind", "gravity", "primaryColor", "glowRadius")
            if k in base
        }
        doc = {
            "id": did,
            "label": label,
            "author": "CoreDesk Engine",
            "version": 1,
            "base": did,
            "params": params,
        }
        (FX_DIR / f"{did}.json").write_text(
            json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
        count += 1
    print(f"effects: {count} preset files -> {FX_DIR}")


if __name__ == "__main__":
    split_clock()
    split_visualizer()
    emit_effect_presets()
