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
