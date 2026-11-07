#!/usr/bin/env python3
"""Shared per-style file emitter for the data-driven style catalogs.

Every selectable style is one user-editable JSON document:
`Styles/<category>/<id>.json` — `{id, label, author, version, ...payload}`.
The file stem IS the catalog id (the string `settings.*_style` and `.cde`
archives store); `label`/`author`/`version` are display metadata; the rest is
the mod-converted payload verbatim (`styleConstants`, `targetStyles`,
`themeResourceVariables`, `startMenuLayout`, `classicVariant`,
`variantFeature`, `weatherVariant`, `webViewTargetStyles` — whichever the
surface ships).
"""

import json
from pathlib import Path

# The three fixed category directory names — kept in one place so the
# import/export scripts and the runtime (crate::styles_fs) agree.
CATEGORY_DIRS = {
    "taskbar": "Taskbar style",
    "start_menu": "Start Menu style",
    "notification_center": "Notification Center style",
}


def write_category(catalog: dict, category_dir: str, out_root) -> int:
    """Emit one `<id>.json` per theme under `out_root/<category_dir>/`.

    `catalog` is the importer's merged shape
    (`{"options": [{id,label}], "themes": {id: payload}}`); `options` only
    provides labels — the file set itself is the catalog.
    """
    labels = {o["id"]: o["label"] for o in catalog.get("options", []) if o.get("id")}
    cat_dir = Path(out_root) / category_dir
    cat_dir.mkdir(parents=True, exist_ok=True)
    for sid, payload in catalog.get("themes", {}).items():
        doc = {
            "id": sid,
            "label": labels.get(sid, sid),
            "author": "CoreDesk Engine",
            "version": 1,
        }
        doc.update(payload)
        (cat_dir / f"{sid}.json").write_text(
            json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    return len(catalog.get("themes", {}))
