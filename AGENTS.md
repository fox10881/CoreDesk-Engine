# CoreDesk Engine Agent Rules

## Arabic responses — formatting English terms

The user is a native Arabic speaker who understands English as a second
language. When writing Arabic, mixing inline English/Latin terms breaks the
right-to-left word order (bidi rendering scrambles the sentence). Rule:
always wrap English technical terms — identifiers, API names, file names,
config keys — inside parentheses or quotation marks (e.g. ملف
(`video-*.mp4`) أو إعداد `"wallpaperResCap"`) so the Arabic sentence stays
correctly ordered. Never leave bare English terms mid-sentence.

## External consultation — DISABLED

Do **not** call the `deepseek-v4-pro` MCP server (`deepseek_consult` or any
other tool on that server). The account has no credit — every call fails or
costs money the user does not want to spend. This is a permanent ban, not a
preference: never invoke it, even for architectural reviews. Do the analysis
and self-review locally instead.

## Granular commits — commit and push every change immediately

From 2026-11-16 onward: every change, however small, gets its own commit that
is committed AND pushed to `main` right away — no batching multiple edits into
one commit, no leaving work uncommitted at the end of a session. One logical
change = one commit. The user wants their contribution count on GitHub to grow
steadily with real work; granular commits also give a cleaner bisectable
history. Applies to every kind of edit: code fixes, docs, config, rules files.
Never create empty or meaningless commits — every commit must carry a real
change.

Commit messages carry NO Devin signature: no "Generated with Devin" line and
no "Co-Authored-By: Devin" trailer. Commits are authored solely under the
user's identity so the repository shows a single contributor.
