"""Generate categorized release notes from commits between two refs.

Categorizes each commit subject into Features / Fixes / Performance / Docs /
Other by leading keyword, then emits a markdown body suitable for a GitHub
release. Used by release.yml to replace --generate-notes; runnable locally:

    python scripts/release_notes.py --to v1.3.4 --repo fox10881/CoreDesk-Engine
"""
import argparse
import re
import subprocess
import sys

GROUPS = [
    ("✨ Features", re.compile(r"^(add|feat|implement|new|support|introduce|enable)", re.I)),
    ("🐛 Fixes", re.compile(r"^(fix|repair|resolve|prevent|correct|stop|restore)", re.I)),
    ("⚡ Performance", re.compile(r"^(perf|speed|optimi[sz]e|faster|reduce|cache)", re.I)),
]
DOC_AREAS = {"readme", "docs", "doc", "agents", "ci", "changelog", "build", "release"}


def sh(args):
    return subprocess.run(args, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.strip()


def commits_between(frm, to):
    rng = f"{frm}..{to}" if frm else to
    out = sh(["git", "log", rng, "--format=%h%x09%s"])
    return [line.split("\t", 1) for line in out.splitlines() if "\t" in line]


def previous_tag(to):
    tags = sh(["git", "tag", "--sort=-creatordate"]).splitlines()
    tags = [t for t in tags if t]
    if to in tags:
        idx = tags.index(to)
        return tags[idx + 1] if idx + 1 < len(tags) else ""
    return tags[0] if tags else ""


def build_notes(commits, repo, frm, to):
    buckets = {title: [] for title, _ in GROUPS}
    buckets["📝 Docs & Chores"] = []
    other = []
    area = re.compile(r"^([A-Za-z][A-Za-z-]*):\s*(.*)$")
    for sha, subject in commits:
        m = area.match(subject)
        if m and m.group(1).lower() in DOC_AREAS:
            buckets["📝 Docs & Chores"].append((sha, subject))
            continue
        verb_target = m.group(2) if m else subject
        for title, pattern in GROUPS:
            if pattern.match(verb_target):
                buckets[title].append((sha, subject))
                break
        else:
            other.append((sha, subject))
    lines = ["## What's Changed", ""]
    for title in [t for t, _ in GROUPS] + ["📝 Docs & Chores"]:
        if buckets[title]:
            lines.append(f"### {title}")
            lines += [f"- {s} ({h})" for h, s in buckets[title]]
            lines.append("")
    if other:
        lines.append("### Other changes")
        lines += [f"- {s} ({h})" for h, s in other]
        lines.append("")
    if repo and frm:
        lines.append(f"**Full Changelog**: https://github.com/{repo}/compare/{frm}...{to}")
    return "\n".join(lines).rstrip() + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--to", default="HEAD", help="tag/ref the notes describe")
    ap.add_argument("--from", dest="frm", default=None, help="start ref (default: previous tag)")
    ap.add_argument("--repo", default="", help="owner/repo for the compare link")
    ap.add_argument("--out", default="", help="write notes to this file instead of stdout")
    args = ap.parse_args()
    frm = args.frm if args.frm is not None else previous_tag(args.to)
    notes = build_notes(commits_between(frm, args.to), args.repo, frm, args.to)
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as f:
            f.write(notes)
    else:
        sys.stdout.write(notes)


if __name__ == "__main__":
    main()
