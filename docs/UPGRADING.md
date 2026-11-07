# Upgrading to CoreDesk Engine

CoreDesk Engine is the new name of this project. The executable is `coredesk.exe`,
with `coredesk-watchdog.exe` beside it.

## Data locations

Configuration lives in `%APPDATA%\CoreDeskEngine`; logs and WebView2 profiles in
`%LOCALAPPDATA%\CoreDeskEngine`. Both are created fresh on first launch.
Portable mode uses the `config` directory beside the executable instead.

## Switching between the ZIP and the installer

Releases offer `coredesk-v<version>-x64-portable.zip` and
`coredesk-v<version>-x64-setup.exe`, with the same program inside. Both keep
settings in `%APPDATA%\CoreDeskEngine`, so switching keeps your fences, rules and
settings. Exit CoreDesk Engine from its tray menu first.

- **ZIP to installer:** run the setup EXE. It can install into the folder you
  extracted the ZIP into and replaces the program files there. If you install
  somewhere else, you can delete the old ZIP folder afterwards.
- **Installer to ZIP:** uninstall through Windows Settings (your data is kept),
  then extract the ZIP and run `coredesk.exe`.
- If you started the ZIP with `--portable`, its configuration is in the `config`
  folder beside the executable, which other copies do not read. Use **Export or
  import configuration** in Settings to carry it over.

## Windows startup

A normal release launch registers the `CoreDeskEngine` startup entry when required.
When startup is on, the running release points the entry at its own executable, so
the copy that ran last starts with Windows.

Portable and development launches do not change startup entries. Changing the
autostart switch in Settings remains an explicit opt-in/out.

## Environment overrides

Use the `COREDESK_` prefix for application overrides, for example
`COREDESK_INSTANCE` and `COREDESK_ACRYLIC`.
