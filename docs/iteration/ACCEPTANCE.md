# Acceptance: Real App Verification

> **中文摘要**：code review 通过 ≠ 验收通过。用真实 release exe + 隔离 demo 数据，
> 把每条 AC 跑一遍，留截图 / 命令 / verdict，最后由你签字。

Code review answers "does the diff follow the standards and the spec?".
Acceptance answers "does the real application, built the real way, do what the
user needs?". They are separate gates. A green test suite is not acceptance.

## Preconditions

- `ppclip.exe` is not running (otherwise the build hits `os error 5` and a second
  launch just wakes the existing instance).
- `dist/` exists before any Rust build.
- Use an isolated `PPCLIP_DATA_DIR` for demo data and smoke runs; never point
  acceptance at the maintainer's real data directory.
- Do not put real clipboard contents into screenshots or committed evidence.
  Use synthetic text, generated images, and sample files.
- Get explicit consent before running smoke flags that move the real cursor or
  steal foreground focus, and before capture flows that overwrite the clipboard.

## Automated checks

Run from the repository root:

```powershell
npm run lint
npm run build
cd src-tauri; cargo test
cd src-tauri; cargo check
```

Release build (long; required for UI acceptance):

```powershell
npm run build
cd src-tauri; cargo build --release
```

Smoke flags (each has side effects):

| Flag | What it verifies | Side effects |
|---|---|---|
| `--capture-fullscreen` | clipboard + history write, hides own windows | overwrites the clipboard; writes history |
| `--capture-region` | overlay creation + frontend show | creates a full-screen overlay; the smoke exits automatically, a real selection needs a human |
| `--smoke-wheel` | wheel expands/collapses, geometry, anchor | creates topmost windows; brief UI activity |
| `--smoke-dock` | drag/dock/undock/redock | **moves the real cursor** |
| `--smoke-panel` | panel pin/blur/second instance | **steals foreground focus** |

The release executable is `src-tauri\target\release\ppclip.exe`. Run smoke tests
with an isolated data directory, for example:

```powershell
$env:PPCLIP_DATA_DIR = "$env:TEMP\ppclip-acceptance"
.\src-tauri\target\release\ppclip.exe --smoke-wheel
```

Record the exact command and exit code in `acceptance.md`.

## Real-app flows

At minimum, exercise the flows touched by the batch:

### Panel and history
- Open/close via the ball and the tray.
- Search, clear search, switch All/Text/Images/Files.
- Select with mouse and keyboard; open the viewer.
- Copy, pin/unpin, delete (two-click confirmation), clear all.
- Empty state, no-search-results state, long text, image thumbnail, single-file
  and multi-file rows.
- Panel pinned (stays on top, does not close on outside click) and unpinned
  (closes on focus loss).
- Settings screen, About page, opacity slider extremes, data-directory button.

### Viewer
- Text: selection and copy; footer does not push `Esc` off-screen at narrow
  widths.
- Image: fit / actual size, double-click toggle, zoom controls, scroll range.
- File: type/size/modified time, path block, reveal, missing-file state, hash
  calculation for SHA-256 and MD5.
- Single window: opening a second clip replaces the content, not a new window.

### Overlay and screenshots
- Drag a region, adjust it, confirm with Enter/double-click, cancel with Esc /
  right-click.
- Re-draw a selection, move the whole selection, check the size label.
- Fullscreen capture on the monitor under the cursor.
- After capture, the panel/ball are restored and the result is in history.

### Ball and tray
- Tab ↔ orb hover; 2-second wheel; wheel actions close the wheel before acting.
- Drag to edge → dock; leave → slide back; bad drag in the middle → no dock.
- New-item badge; opacity setting; hidden ball can be restored from the tray.
- Tray left-click toggles the panel; right-click menu items work.

### Pins
- Pin text and image; drag, zoom, close; restart restores position/size.
- Pinning the same clip twice toggles instead of duplicating.
- File clips have no pin action.

### Persistence
- Panel position, ball anchor/docked state, opacities, panel pinned state,
  history, and pins survive a restart.
- Deleting a clip closes its viewer/pin; clearing history closes pin windows.

## Manual edge cases

- 100% and 150% DPI.
- Multi-monitor: selection only on the cursor's monitor; viewer placement.
- Data directory on a permission-denied or read-only path.
- Deleted/moved files in a file clip.
- Very large image, very long text, history near the item limit.
- Autostart toggle in a restricted-token environment (must report the error,
  not silently claim success).
- Fullscreen exclusive app covering the ball/pin (known limitation; confirm it
  fails gracefully).
- Network failure during the About page update check (must show a readable
  error, not hang).

## Acceptance matrix

Write `docs/iteration/batches/<id>/acceptance.md`:

| Issue | Requirement | Acceptance criterion | Evidence | Verdict | Notes |
|---|---|---|---|---|---|
| #12 | ... | ... | screenshot / log / command | verified / partial / failed | residual risk |

Verdicts:

- **verified**: the criterion is met, evidence is attached, no regression in the
  touched area.
- **partial**: some evidence is missing or a residual risk remains; it must be
  explicitly accepted by the maintainer and does not ship as-is.
- **failed**: not met; reopen the task/issue and return to development.

## Evidence

- Screenshots at native resolution, named
  `<version>-<surface>-<state>.png`.
- `manifest.json` per capture set: version, git commit, build kind, DPI, screen
  size, window rectangles, data directory, capture method.
- Log excerpts only when they contain no clipboard content or personal paths.
- Commands and exit codes for smoke tests.
- Do not commit temporary data directories or `.runtime/`.

## Sign-off and release gate

- The maintainer signs `acceptance.md` (date + initials/name).
- No release with a `failed` item or an unaccepted `partial`.
- Release gate: all selected acceptance criteria `verified`; no S1/S2 defect
  open; code-review blocking findings zero; version files consistent; docs and
  release notes updated.

## Tooling status

The automated acceptance script (`acceptance.ps1`) and the screenshot harness
are **planned**, not part of this documentation round. Until they exist, use
UI Automation, screen capture, and the manual checklist above, and record the
evidence by hand. Do not claim a script exists when it does not.
