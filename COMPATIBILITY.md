# Windows Build Compatibility

Which Windows builds each CoreDesk release was verified on. Shell styling
targets XAML surfaces whose element names and lifetimes drift between
Windows builds, so this matrix is the canonical answer to *"is my build
supported?"*.

The app also protects itself at runtime: on a build it has never seen, the
taskbar styler runs a read-only probe before the first theme write, and the
`NoDiag` crash breaker is keyed per build + UBR + resident image — a bad
combination bans itself automatically. This file is the *public* record on
top of that self-healing.

## Verified builds

| App version | Windows build | UBR | Result | Date | Source |
|---|---|---|---|---|---|
| 1.3.4 | 26300 | 9974 | Stable — taskbar/Start/Notification Center/Quick Settings all styled, zero `NoDiag` trips | 2026-11-17 | dev |

`Source` is `dev` (verified on the maintainer's machines before release) or
`community` (reported by users via [CoreDesk-feedback](https://github.com/fox10881/CoreDesk-feedback)).
Older releases predate this matrix; their verification status is unknown —
treat them as untested rather than broken.

## Style notes

Per-style exceptions where a specific theme is *proven* to resolve
differently across builds. This is intentionally short — only recorded
findings, not a per-style matrix.

- **macOS Tahoe Midnight** — on build 26300 the modern tray host
  (`TaskbarExtensionElement`) does not expose the legacy `SystemTray.*`
  element names the theme targets, so clock-font/date-collapse rules do not
  resolve. The tray pill and transparency still apply.

## Known issues

Windows-update-induced regressions in shipped versions, once proven.

*None recorded yet.*
