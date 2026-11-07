#!/usr/bin/env python3
"""Split the embedded style catalogs (`crates/core/data/*_themes.json`) into
per-style files under `styles/` — the shipped factory set the installers
deploy to `{app}\\Styles\\`.

The import-*-themes.py scripts emit these files directly now; this script is
the one-shot extraction from the already-generated merged catalogs.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from style_emit import CATEGORY_DIRS, write_category

DATA = Path("crates/core/data")
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("styles")

SOURCES = {
    "taskbar": "taskbar_themes.json",
    "start_menu": "start_menu_themes.json",
    "notification_center": "notification_center_themes.json",
}

total = 0
for surface, filename in SOURCES.items():
    catalog = json.loads((DATA / filename).read_text(encoding="utf-8"))
    n = write_category(catalog, CATEGORY_DIRS[surface], OUT)
    print(f"{CATEGORY_DIRS[surface]}: {n} style files")
    total += n
print(f"{total} style files -> {OUT}")
