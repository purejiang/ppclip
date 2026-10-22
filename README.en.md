<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" alt="PPClip" />
</p>

<h1 align="center">PPClip</h1>

<p align="center">
  <b>Clipboard history · Screenshots · Desktop pins — for Windows</b><br />
  Everything you copy — text, images, and files — lands in a searchable history. Take region or full-screen screenshots, and pin what you need to keep in sight.
</p>

<p align="center">
  <a href="README.md">简体中文</a> · English
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4?logo=windows&logoColor=white" alt="Windows 10/11" />
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React 19" />
  <img src="https://img.shields.io/badge/Rust-1.77%2B-000000?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/License-MIT-3DA639" alt="MIT License" />
</p>

<p align="center">
  <img src="panel-final.png" width="420" alt="PPClip history panel" />
</p>

## Overview

PPClip is a small Windows utility that lives in a floating ball and a tray icon. There is no main window to keep around: click the ball or the tray icon to open your clipboard history. New items are deduplicated by content hash and stored in a local SQLite database. Screenshots are copied to the clipboard and added to the same history.

Everything stays on your machine and is never uploaded or synced. PPClip is offline by default; it contacts GitHub's releases API (api.github.com) only when you press “Check for updates” — no clipboard content is sent, and nothing is downloaded or installed.

## Features

- 📋 **Clipboard history** — text, images, and files (a multi-file selection from Explorer counts as one item); duplicates move to the top instead of being inserted again; delete one item or clear the whole history
- 🔍 **Search & filters** — search by keyword, filter by all / text / image / file
- ✂️ **Screenshots** — drag a region, fine-tune it, then press `Enter` or double-click; `Esc` / right-click cancels. Full-screen capture covers the display under the cursor. Results go straight to the clipboard and history
- 🪄 **Three-state floating ball** — edge capsule ↔ orb ↔ quick wheel; snaps to screen edges and slides back when the cursor leaves; shows a badge for new items
- 📌 **Desktop pins** — pin text or images on the desktop; drag to move, scroll to zoom; position and size are restored after restart
- 👁️ **Viewer** — a single window next to the panel; select and copy text, switch images between fit-to-window and actual size, inspect file type / size / modified time / path, and compute SHA-256 and MD5 on demand
- 🖥️ **Keep on top** — keep the history panel above other windows so it stays put when you click elsewhere; by default it closes when it loses focus
- ⚙️ **Settings** — history limit (300 by default), image storage limit (200 MB by default), data directory, floating ball visibility and opacity, panel opacity, launch at startup
- 🧩 **Extras** — tray icon, single instance (launching again brings the panel back), file logs

## Usage

| Action | What it does |
|---|---|
| Floating ball · click | Open / close the history panel |
| Floating ball · drag | Move it; drop near a screen edge to snap into the capsule |
| Edge capsule · hover | Expands into the orb; hover the orb for 2 seconds to open the quick wheel |
| Tray icon | Left-click toggles the panel; right-click opens the menu |
| Panel · click an item | Opens the item in the viewer window |
| Panel · row buttons | Copy / pin to desktop / delete (delete requires two clicks) |
| Panel · top corners | Settings (left) / keep on top and close (right) |
| Panel · keyboard | `↑` `↓` select · `Enter` details · `Ctrl+Enter` copy · `Esc` close |
| Viewer | Double-click an image to toggle fit / actual size; `Esc` goes back |
| Screenshot | Drag a region → `Enter` or double-click to confirm; `Esc` / right-click cancels |
| Desktop pin | Scroll to zoom; right-click for the menu; close from the top-right button |

Command-line screenshot (useful in scripts):

```powershell
ppclip.exe --capture-fullscreen
```

## Installation

**Requirements:** Windows 10/11 (x64) and the WebView2 runtime (included with Windows 11; usually already present on Windows 10 through Edge).

1. Download the latest `PPClip_x.y.z_x64-setup.exe` from [Releases](https://github.com/purejiang/ppclip/releases)
2. Run the installer (NSIS, per-user, no administrator rights required)
3. Launch PPClip — the floating ball appears on your desktop and the tray icon shows up

> No installer in Releases yet? [Build from source](#build-from-source) instead.

## Build from source

You need Node.js 20.19+, Rust with the MSVC toolchain, and WebView2.

```bash
git clone https://github.com/purejiang/ppclip.git
cd ppclip
npm install

npm run tauri:dev     # development: Vite + Rust + floating ball
npm run tauri:build   # package: NSIS installer
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.

Other commands:

```bash
npm run build         # frontend only (tsc -b && vite build)
npm run lint          # oxlint
cd src-tauri && cargo test    # Rust unit tests
cd src-tauri && cargo check   # Rust type check
```

**Development notes**

- Run `npm run build` before calling `cargo build` / `cargo check` directly; `tauri::generate_context!` expects `dist/` to exist.
- Keep `[profile.dev.package.libsqlite3-sys] debug = false` in `src-tauri/Cargo.toml`; compiling SQLite with debug info fails with MSVC error D8050.
- Close any running `ppclip.exe` before rebuilding, otherwise you will hit os error 5 (file in use).

## Data & privacy

All data is stored locally and is never uploaded or synced. PPClip makes no background network requests; the only outbound request is a GitHub Releases API call (api.github.com) when you press “Check for updates”. It sends no clipboard content and downloads or installs nothing.

Default location: `%APPDATA%\com.purejiang.ppclip\`

| Path | Contents |
|---|---|
| `ppclip.db` | SQLite database (clips / pins) |
| `images/`, `thumbs/` | Original images and thumbnails, named by content SHA-256 |
| `settings.json` | Limits, window positions, and other settings |
| `logs/ppclip.log` | Runtime log |

## Tech stack

Tauri 2 · React 19 · TypeScript · Vite · Rust · rusqlite / SQLite

The UI has no component library and no icon font: styles are hand-written CSS variables, and icons are hand-written SVG components.

## Known limitations

- No global hotkey: use the floating ball or the tray icon. If you hide the ball, bring it back from the tray menu.
- The tray icon does not show a new-item count; the badge is only on the floating ball.
- Screenshots do not support annotations, OCR, window capture, or scrolling capture.
- File items can be viewed and copied, but not pinned to the desktop.
- Only plain text, images, and file lists are stored; rich text / HTML is not preserved.
- On multi-monitor setups, a selection is limited to the display under the cursor (mixed-DPI cross-screen selection is not supported).
- Exclusive full-screen apps (some games and video players) may cover the floating ball and desktop pins.
- Check for updates requires network access; a failed check does not affect any local feature.
- The interface is currently Chinese-only.

## Contributing

Issues and pull requests are welcome. Before opening a PR, please run `npm run lint`, `npm run build`, and `cd src-tauri && cargo test`.

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## License

MIT
