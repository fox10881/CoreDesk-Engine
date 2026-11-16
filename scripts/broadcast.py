"""Broadcasting agent: turn a release's notes into per-platform posts.

Generates ready-to-post drafts for Reddit, X, Discord and DeviantArt under
out/broadcast/, and can post directly where credentials exist:

    secrets/reddit.json   {"client_id","client_secret","username","password","subreddit"}
    secrets/discord.json  {"webhook_url"}

Drafts only:     python scripts/broadcast.py --to v1.3.4
Post too:        python scripts/broadcast.py --to v1.3.4 --post reddit --post discord
"""
import argparse
import base64
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = "fox10881/CoreDesk-Engine"
KOFI = "https://ko-fi.com/fox881"


def notes_for(tag):
    out = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "release_notes.py"), "--to", tag, "--repo", REPO],
        capture_output=True, text=True, encoding="utf-8").stdout
    return out.strip()


def bullets(notes, section):
    m = re.search(rf"### {re.escape(section)}\n((?:- .+\n?)+)", notes)
    return [l[2:].strip() for l in m.group(1).splitlines()] if m else []


def release_url(tag):
    return f"https://github.com/{REPO}/releases/tag/{tag}"


def drafts(tag, notes):
    feats = bullets(notes, "✨ Features") or bullets(notes, "Other changes")
    fixes = bullets(notes, "🐛 Fixes")
    feat_lines = "\n".join(f"- {re.sub(r' \(\w+\)$', '', f)}" for f in feats[:8])
    fix_lines = "\n".join(f"- {re.sub(r' \(\w+\)$', '', f)}" for f in fixes[:5])

    reddit = f"""Title: CoreDesk {tag} — Windows 11 shell customization suite (open source, Rust)

{r'Hey r/desktops — CoreDesk Engine ' + tag + ' is out. It’s a Rust-native Windows 11 desktop suite: icon fences, live H.264 video wallpapers, Direct2D particle effects, and deep shell theming (taskbar, Start menu, Notification Center, File Explorer).'}

Highlights:
{feat_lines}
{('Notable fixes:\n' + fix_lines) if fix_lines else ''}
Everything is one portable binary or an Inno installer, no services left behind. The shell styler works through XAML diagnostics injection with a crash-guard that disarms automatically if Explorer respawns too fast — happy to go into the details if anyone's curious about that part.

Download: {release_url(tag)}
Source: https://github.com/{REPO} (Apache-2.0)
Tip jar: {KOFI}
"""

    tweets = [
        f"CoreDesk {tag} is out — a Rust-native Windows 11 customization suite: fences, video wallpapers, Direct2D effects, taskbar/Start/Explorer theming. Free & open source. 🧵",
        *[f"• {re.sub(r' \(\w+\)$', '', f)}" for f in feats[:4]],
        f"Download {tag}: {release_url(tag)}\nSupport: {KOFI}",
    ]
    x_thread = "\n\n".join(f"{i+1}/{len(tweets)}\n{t}" for i, t in enumerate(tweets))

    discord = f"""@everyone **CoreDesk {tag}** is live 🎉

{feat_lines}
{('Fixed:\n' + fix_lines) if fix_lines else ''}
📦 {release_url(tag)}
☕ {KOFI}"""

    deviantart = f"""CoreDesk {tag} theme — built for the CoreDesk Engine desktop suite.

Import the .cde archive in Settings → Appearance → Import theme. All styles, wallpapers and widgets apply at once.

Engine (free): https://github.com/{REPO}
More themes & previews: https://www.deviantart.com/fox881
Support: {KOFI}"""

    return {"reddit.md": reddit, "x_thread.txt": x_thread, "discord.txt": discord, "deviantart.txt": deviantart}


def post_json(url, payload, headers=None):
    req = urllib.request.Request(url, json.dumps(payload).encode(),
                                 {"Content-Type": "application/json", **(headers or {})})
    return urllib.request.urlopen(req, timeout=30)


def post_reddit(title, body):
    creds = json.loads((ROOT / "secrets" / "reddit.json").read_text(encoding="utf-8"))
    auth = base64.b64encode(f"{creds['client_id']}:{creds['client_secret']}".encode()).decode()
    token_req = urllib.request.Request(
        "https://www.reddit.com/api/v1/access_token",
        f"grant_type=password&username={creds['username']}&password={creds['password']}".encode(),
        {"Authorization": f"Basic {auth}", "User-Agent": "CoreDeskBroadcast/1.0"})
    token = json.loads(urllib.request.urlopen(token_req, timeout=30).read())["access_token"]
    payload = {"sr": creds["subreddit"], "title": title, "text": body, "kind": "self", "api_type": "json"}
    req = urllib.request.Request("https://oauth.reddit.com/api/submit", json.dumps(payload).encode(),
                                 {"Authorization": f"Bearer {token}", "User-Agent": "CoreDeskBroadcast/1.0",
                                  "Content-Type": "application/json"})
    resp = json.loads(urllib.request.urlopen(req, timeout=30).read())
    return resp["json"]["data"].get("url", json.dumps(resp)[:300])


def post_discord(content):
    url = json.loads((ROOT / "secrets" / "discord.json").read_text(encoding="utf-8"))["webhook_url"]
    post_json(url, {"content": content})
    return "sent"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--to", required=True, help="release tag the posts describe")
    ap.add_argument("--notes", default="", help="markdown notes file (default: generated)")
    ap.add_argument("--out", default="out/broadcast")
    ap.add_argument("--post", action="append", choices=["reddit", "discord"], default=[])
    args = ap.parse_args()

    notes = Path(args.notes).read_text(encoding="utf-8") if args.notes else notes_for(args.to)
    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    files = drafts(args.to, notes)
    for name, text in files.items():
        (out / name).write_text(text, encoding="utf-8")
        print(f"wrote {out/name}")

    if "discord" in args.post:
        print("discord:", post_discord(files["discord.txt"]))
    if "reddit" in args.post:
        title, _, body = files["reddit.md"].partition("\n")
        print("reddit:", post_reddit(title.replace("Title: ", ""), body.strip()))


if __name__ == "__main__":
    main()
