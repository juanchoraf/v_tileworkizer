<p align="center"><img src="assets/logo.svg" width="112" alt="v_tileworkizer logo"></p>

# v_tileworkizer

[![Release_Badge]][Release_Url]
[![Build_Badge]][Build_Url]
[![Dependencies_Badge]][Dependencies_Url]
![License_Badge](https://img.shields.io/badge/License-Apache--2.0_or_MIT-blue)

A Rust desktop tiling workspace organizer with a native GUI, command-line controls,
and a persistent background service.

Made with AI (Codex) 🤖

## Supported OS

| Platform  | Support |
| --------- |:-------:|
| Linux     |   ✅    |
| Windows   |   ✅    |
| macOS     |   ✅    |
| Unix      |   ✅    |

## Features

- **Native desktop interface:** Workspace and Settings views, a dark theme, layout
  previews, save status, and success/error notifications.
- **Four standard layouts:** Master, Columns, Rows, and Grid, with configurable
  window gaps, display insets, and master-window share.
- **Independent layouts per display:** Choose a standard layout or custom preset
  for each monitor and switch between displays in the workspace editor.
- **Live desktop discovery:** Automatically refreshed display, window, and saved
  preset counts, plus an inventory of window titles, IDs, states, and displays.
- **Display identification:** Monitor manufacturer labels where available and
  temporary numbered overlays to match physical screens to the display selector.
- **Arrange Once:** Apply the selected display's layout without enabling ongoing
  automatic arrangement.
- **Apply & Enforce:** Keep windows arranged in the background on selected displays;
  automatic enforcement starts disabled until requested.
- **Stop enforcement and restore:** Use Not Enforce to restore the selected
  display's previous window arrangement while other displays keep their settings.
  Restoration includes geometry, window state, and stacking where supported; the
  saved restoration baseline lasts for the worker's lifetime.
- **Custom layout presets:** Create, rename, duplicate, and delete up to 64 presets,
  including copies of standard layouts, with up to 64 tiles per preset.
- **Visual tile editing:** Drag tiles to move them, resize from corners, snap nearby
  edges, and deliberately overlap tiles. Use arrow keys to move the selected tile
  and Shift+Arrow for finer adjustments.
- **Precise tile controls:** Name tiles and edit their position and size as display
  percentages. Custom layouts use the exact tile borders drawn in the editor.
- **Overlapping window layers:** Set each tile's layer to control front-to-back
  placement without permanently marking windows as always on top.
- **Automatic or explicit window assignment:** Let layouts distribute windows or
  assign specific windows to individual tiles in standard and custom layouts.
- **Persistent window matching:** Track live window identity through title changes
  and reconnect assignments by application identity after restart. Optional stable
  title fragments distinguish multiple windows from the same application;
  ambiguous matches wait for a selection.
- **Window exclusions:** Ignore case-insensitive title fragments and automatically
  leave minimized, fullscreen, hidden, and protected windows alone where supported.
  Assigned unavailable windows wait until eligible again.
- **Automatic saving:** Save custom layout edits and settings, retain editor drafts
  while switching layouts/displays, and store independent configuration per user.
  Configuration validation and atomic writes protect saved data and preserve unknown
  configuration fields.
- **Tunable background operation:** Set the refresh interval from 250 ms to 30 seconds,
  reset general settings to defaults, and restart a stopped service from Settings.
- **Persistent session service:** Continue arranging windows after the GUI closes,
  start at desktop login through installer registration, and supervise the worker
  so it restarts after unexpected exits.
- **Command-line controls:** Open the GUI with no arguments or `--gui`; arrange with
  `--tile`, pause and restore with `--pause`, resume with `--resume`, and manage the
  worker with `--service`, `--supervise`, and `--stop-service`.
- **Diagnostics:** Inspect desktop inventory as JSON with `--inspect`, view service
  health and configuration location with `--status`, and access `--help` and
  `--version`.
- **GitHub updates:** Use the sidebar Update button or `--check-update` / `--update`
  exclusively with `juanchoraf/v_tileworkizer` releases. The update modal shows
  download progress, verification, installation, and a dismissible result.
- **Verified native installers:** Select updates for the current OS and architecture,
  verify their size and SHA-256 digest, and install through the native package
  system. Packaging targets Linux DEB/RPM, Windows MSI, macOS PKG, and FreeBSD PKG
  with system-wide installation and per-user settings.
- **Platform backends:** Windows, macOS, EWMH-compatible X11 desktops on Linux and
  Unix-like systems, and Sway through its IPC interface. Other Wayland compositors
  are not supported; macOS requires Accessibility permission.

---

## Updates

`v_tileworkizer` is GUI-only. The bottom-right controls show the installed
version and an Update button. Update checks the release source and installs the
latest matching binary from `https://github.com/juanchoraf/v_tileworkizer` over
the current installation when a newer release is available.

## Credits

Powered by The Velasquez.

## License

The `v_tileworkizer` library is distributed under either of

 * [Apache License, Version 2.0][LICENSE-APACHE]
 * [MIT license][LICENSE-MIT]

at your convenience.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[Release_Badge]: https://github.com/juanchoraf/v_tileworkizer/actions/workflows/release.yml/badge.svg
[Release_Url]: https://github.com/juanchoraf/v_tileworkizer/actions/workflows/release.yml
[Build_Badge]: https://github.com/juanchoraf/v_tileworkizer/actions/workflows/rust.yml/badge.svg?branch=main
[Build_Url]: https://github.com/juanchoraf/v_tileworkizer/actions?query=branch:main
[Dependencies_Badge]: https://deps.rs/repo/github/juanchoraf/v_tileworkizer/status.svg
[Dependencies_Url]: https://deps.rs/repo/github/juanchoraf/v_tileworkizer

[//]: # (licenses)

[LICENSE-APACHE]: https://github.com/juanchoraf/v_tileworkizer/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/juanchoraf/v_tileworkizer/blob/main/LICENSE-MIT
