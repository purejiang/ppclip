# Requirements — 2026-10-22 (local dry-run)

- **Status**: draft for maintainer approval. No code has been written.
- **Mode**: local dry-run; candidates are local IDs, not GitHub issues.
- **Inputs**: `intake.md`, `triage.md`, `council.md`, `research.md`.
- **Council recommendation**: pilot C-01, evidence C-04; C-02 to be sequenced
  after C-01; C-03/L-03 deferred.
- **Lead proposal**: select **R-01**, **R-02**, **R-03**; defer L-03, undo/trash,
  and the pin-exemption policy.

## Problem Statement

Clearing clipboard history is destructive and currently happens on a single
click. The same click also closes every pinned desktop window and deletes image
and thumbnail files from disk. There is no confirmation and no undo. The panel's
single-clip delete already uses a two-step confirmation, so clear-all breaks the
product's own destructive-action convention.

Separately, both READMEs claim the app makes no network requests, while the About
page can send one user-triggered GitHub Releases request. The claim is false as
written, in both languages, and the point of action does not disclose it.

L-03 (false empty state) remains a hypothesis: the ordinary first open usually
mounts long before the panel is revealed. The reachable path is pinned startup;
there is no reproduction yet, so it is not in this batch.

## Solution

1. **R-01**: Make clear-all a two-step action. The first activation arms an
   inline confirmation strip above the status bar and deletes nothing. The strip
   states the clip count, that pinned windows will close, that image/thumbnail
   files are deleted, and that it cannot be undone. Confirm calls the existing
   `clear_clips`; cancel/`Esc` leaves everything untouched.
2. **R-02**: Reword every absolute offline claim in both READMEs and the About
   page to one accurate, scoped statement: data stays local, offline by default,
   one manual GitHub request only when the user presses 检查更新, nothing uploaded
   from the clipboard, nothing downloaded or installed.
3. **R-03**: Add the missing Rust regression test for `clear()` cascading pins,
   and a repeatable acceptance runbook using an isolated `PPCLIP_DATA_DIR`.

## Requirement R-01 — Clear-all confirmation

| Field | Value |
|---|---|
| Type | FIX |
| Source | L-01 |
| Priority | P0 |
| Severity | S1 (data loss) |
| Effort | S |
| Risk | low |
| Council candidates | C-01 (chair), C-UX-01…03, C-E1, security C-01 |

### Evidence

- `src/windows/PanelApp.tsx:285-295` — `clearAll()` calls `api.clearClips("all")`
  immediately; no confirmation.
- `src/windows/PanelApp.tsx:457-466` — the eraser button calls it on first click.
- `src/windows/PanelApp.tsx:91-98` — single-clip delete already uses a two-step
  confirmation (house convention).
- `src-tauri/src/commands.rs:418-455` — `clear_clips` → `storage.clear`, then
  closes affected pin windows and the viewer.
- `src-tauri/src/storage.rs:35` — `pins.clip_id REFERENCES clips(id) ON DELETE
  CASCADE`.
- `src-tauri/src/storage.rs:309-340` — `clear()` deletes rows and image/thumb
  files from disk.
- `AGENTS.md` decision 5/16 — eviction protects pins; clear-all currently does
  not.
- Research S1/S2/S3/S4/S5/S6/S7/S8 — confirmation copy, focus, Esc, contrast,
  status messages, reduced motion.

### User stories

1. As a user, when I click the eraser, I want to see what will be deleted before
   anything happens.
2. As a user with pinned windows, I want to know that clearing history also
   closes them, so I can cancel.
3. As a keyboard user, I want focus on the safe action, `Esc` to cancel, and
   `Enter` not to wipe history by reflex.
4. As a screen-reader user, I want the result announced after a successful clear.
5. As a user who hides the panel mid-confirmation, I want the next summon to
   start unarmed, not hold a stale confirmation.

### UX flow

1. The eraser is disabled while `stats` is unknown or `stats.total === 0`; its
   title/`aria-label` explains why. This avoids a false `0` and an empty confirm.
2. First click: arm the confirmation. No API call. The strip appears above the
   status bar; the clip list shrinks by the strip height. The rest of the panel
   is inert (or equivalent: no pointer/focus targets) while armed.
3. Initial focus is the **取消** button. The **取消** control occupies the
   eraser's old far-right position, so the second click of a double-click lands
   on cancel, not confirm. The destructive button is offset left.
4. Copy (two lines, 11/10 scale):
   - Known counts: 「清空全部历史？将删除 N 条记录，并关闭 M 个桌面贴图；磁盘上的
     图片文件也会删除。此操作无法撤销。」
   - Pin list unknown: 「…已贴到桌面的内容也会关闭…」
   - Stats unknown: 「…将删除全部记录…」
   - Secondary line: 「文件条目的源文件不会被删除。」
   - Buttons: 「取消」 / 「清空 N 条」（或「清空全部」）.
5. `Enter` activates the focused button only; it must not confirm globally.
   `Esc` cancels at this layer and does not hide the panel. `Tab`/`Shift+Tab`
   cycle between the two buttons. Arrow keys do nothing while armed.
6. Clicking outside the strip cancels. The header, search, filters, list, and
   status-bar tools cannot be activated while armed.
7. Confirm: disable the buttons, call the unchanged `api.clearClips("all")`.
   On success: close the strip, refresh, and announce `已清空 N 条` via a
   `role="status"` live region; focus moves to the search input (the eraser is
   disabled because history is now empty).
8. On failure: keep the strip open, show the error in the existing panel error
   strip, re-enable the buttons; no data is changed. The user can retry or cancel.
9. Hiding the panel while armed drops the confirmation; the next summon starts
   unarmed. Opening settings/About while armed is not possible (inert).
10. Footer hint: while armed, the right side reads `Esc 取消`; otherwise the
    existing `Esc 关闭`.

### Implementation decisions

- Frontend-only in `src/windows/PanelApp.tsx` and `src/styles/index.css`.
  `clear_clips`, `storage.rs`, and `pins.rs` behavior do not change.
- Add a confirmation state (e.g. `clearConfirm: boolean`) and reuse the existing
  `clearing` flag. Add a `pinsLoaded` boolean to `refresh()` so the copy never
  reports a false `0` pins when `listPins()` failed.
- Add a `role="status"` element for the clear result (the panel has no live
  region today; the viewer toast is a separate window).
- Add a scoped `:focus-visible` rule for the confirmation buttons (2px, at least
  3:1 change). Do not retrofit the whole app in this batch.
- Add a reduced-motion guard for the strip's transition (the stylesheet has none
  today).
- Add `clearConfirm` to the panel keydown effect dependencies; put the
  confirmation branch before About/settings/hide.
- Reset the confirmation on `api.onPanelVisible(false)` (and on unmount).
- Keep the row-delete two-step mechanism untouched; clear-all is a stronger
  action with a different, timer-free confirmation.

### Acceptance criteria

1. **Given** history and pins, **when** the eraser is clicked once, **then** the
   confirmation strip is visible, no clip/pin/file is changed, and focus is on
   `取消`.
2. **Given** the strip is armed, **when** `取消` is clicked or `Esc` is pressed,
   **then** the strip closes, history and pins are untouched, and focus returns
   to the eraser.
3. **Given** the strip is armed, **when** `清空 N 条` is clicked, **then** all
   clips are deleted, pins are cascaded and their windows closed, the viewer is
   closed, image/thumbnail files are removed, the strip closes, and a status
   message reports the returned count.
4. **Given** the strip is armed, **when** `Enter` is pressed while `取消` is
   focused, **then** the action is cancelled; `Enter` without a focused
   destructive button never deletes.
5. **Given** the strip is armed, **when** the panel is hidden and reopened,
   **then** no confirmation is armed and no stale count is shown.
6. **Given** keyboard-only use, **when** tabbing, **then** focus stays within the
   two buttons, the focus indicator is visible, and the Esc layer order is
   confirmation → About → settings → hide panel.
7. **Given** `stats` or the pin list is unavailable, **when** the strip is armed,
   **then** the copy uses indeterminate wording and never prints `0`.
8. **Given** the clear command fails, **when** the error returns, **then** no
   data is changed, the strip remains open, and the error is visible.
9. **Given** history is empty, **then** the eraser is disabled and cannot open an
   empty confirmation.

### Testing decisions

- **Rust**: add a unit test in `src-tauri/src/storage.rs` that creates a clip and
  a pin, calls `clear(None)`, and asserts both rows are gone and the image/thumb
  files are removed. This is test-only; no production behavior change.
- **No frontend test framework** in this batch. The UI is verified with UI
  Automation/manual steps and screenshots against an isolated data directory.
- **Manual/UIA script** (`docs/iteration/batches/2026-10-22/acceptance.md`):
  - Isolated `PPCLIP_DATA_DIR`, create text/image/file clips and at least one
    pin; open the panel.
  - Click eraser → assert no change; capture the strip.
  - Cancel and Esc → assert unchanged and focus returned.
  - Re-arm, confirm → assert counts, pin windows closed, viewer closed, images
    deleted, empty state; capture before/after.
  - Hide while armed → reopen → assert unarmed.
  - Keyboard-only pass; focus ring screenshot.
  - Failure path (e.g. make the data directory read-only or inject an error) →
    assert the strip remains and the error is shown.
- Screen-reader check (NVDA/Narrator) is best-effort this batch; record it as a
  residual risk if not run.

### Risks and rollback

- **Click-through**: mitigated by putting `取消` at the eraser's coordinates and
  offsetting the destructive button.
- **Stale counts**: mitigated by disabling the eraser while stats are unknown and
  using indeterminate wording; the post-clear message uses the command's return
  value, not the pre-count.
- **Hidden-not-unmounted panel**: mitigated by the reset-on-hide rule.
- **Focus/contrast**: new `:focus-visible` and danger styling need a real contrast
  check; record it as an acceptance item.
- **No undo**: confirmation is a speed bump, not a safety net. The copy must not
  imply recoverability.
- **Rollback**: revert the `PanelApp.tsx`/`index.css` changes; the Rust test is
  additive and can stay. No data migration.

### Out of scope

- Undo/trash/soft delete.
- Changing whether clear-all cascades pins; the copy follows current behavior.
- Per-kind clear UI.
- A secure-wipe promise.
- Global focus-ring/reduced-motion retrofit.
- Changing row-delete behavior.
- Gating the Rust command for future callers (recorded as a risk; no such caller
  exists today).

## Requirement R-02 — Scoped offline claim

| Field | Value |
|---|---|
| Type | FIX / DOCS |
| Source | L-02 |
| Priority | P1 |
| Severity | S3 (trust) |
| Effort | S |
| Risk | low |

### Evidence

- `README.md:34`, `README.md:112` — absolute “应用不联网 / 不发起网络请求”.
- `README.en.md:32`, `README.en.md:110` — absolute “no network requests”.
- `src/components/AboutPage.tsx:52-68`, `:132-156` — the check runs only from the
  button; idle hint says “看看 GitHub 上有没有新版本”.
- `src/components/AboutPage.tsx:228-230` — “离线可用，数据只保存在本机”.
- `src-tauri/src/github.rs:23-26` — fixed host/path `api.github.com`,
  `/repos/purejiang/ppclip/releases/latest`; user-triggered only.
- `AGENTS.md` release section — no updater, no signed installer, no CI change.

### User stories

1. As a privacy-conscious user, I want an accurate statement about network use.
2. As an English reader, I want the same accuracy as the Chinese README.
3. As a user pressing 检查更新, I want to know before the click that it goes
   online and uploads nothing.

### Required changes

- Replace both absolute claims in `README.md` and `README.en.md` with one scoped
  statement: data stays local, never uploaded or synced; offline by default; one
  GitHub Releases API request only when the user presses 检查更新; no clipboard
  content sent; nothing downloaded or installed.
- Add a known-limitation line: 检查更新需要联网；失败不影响任何本地功能.
- Update the About idle hint to disclose the request before the click:
  “会请求 GitHub 检查版本；不上传剪贴板内容，不自动下载。”
- Align the About page line “离线可用，数据只保存在本机” with the scoped claim.
- Keep the update check; do not add a setting or an opt-out promise.

### Acceptance criteria

1. No absolute “不联网 / 不发起网络请求 / makes no network requests” claim
   remains in either README.
2. Both languages say the same thing, sentence-for-sentence.
3. The About idle hint discloses the request before the click.
4. The feature still works: the check can run, fail gracefully, and open the
   Release page.
5. The known-limitation line is present.

### Testing decisions

- Text review plus a screenshot of the About idle hint. No code tests.
- Run the existing About check once (network may fail; the failure state is
  acceptable) to confirm no behavior change.

### Out of scope

- Removing or automating the update check.
- A network settings screen or an opt-out.
- Humanizing the update-check error copy (separate improvement).

## Requirement R-03 — Acceptance evidence and pin-cascade test

| Field | Value |
|---|---|
| Type | DEBT / QA |
| Source | L-01 |
| Priority | P1 |
| Effort | S |
| Risk | low |

### Deliverables

- Rust unit test in `src-tauri/src/storage.rs` for `clear()` cascading pins and
  removing image/thumb files.
- `docs/iteration/batches/2026-10-22/acceptance.md` runbook with an isolated
  `PPCLIP_DATA_DIR` and the checks listed under R-01.
- Evidence: screenshots, commands/exit codes, and a `verified` / `partial` /
  `failed` verdict.

### Acceptance criteria

- `cargo test` passes, including the new pin-cascade test.
- The acceptance runbook is executed against the real release app and an
  isolated data directory; no real history is touched.
- Every R-01 acceptance criterion has an evidence row and a verdict.

## Deferred / needs-info

### L-03 — false empty state

No requirement this batch. Narrowed reproduction:

- Launch with `PPCLIP_DATA_DIR` set and `panel_pinned: true`; capture the panel in
  roughly the first 200–500 ms, or add temporary `frontend_log` timestamps around
  the first `refresh()`.
- Also verify the two timing-independent false-empty sub-states:
  `stats?.total ?? 0` printing `0 / 300` (`PanelApp.tsx:435`), and a failed
  `refresh()` leaving the placeholder beside the error banner (`:132-134`).
- If reproduced, the likely scope is a `loaded` flag that distinguishes
  loading / empty / filter-empty / no-match / error. Do not build it before
  reproduction.

### Other deferred items

- Undo/trash/soft delete.
- Exempting pinned clips from clear-all.
- Humanizing the update-check error copy.
- Global `:focus-visible` / `prefers-reduced-motion` retrofit.
- Gating `clear_clips` in Rust for future callers.

## Testing decisions (batch level)

- Existing seams: Rust unit tests (`cargo test`), smoke flags, and manual/UIA
  acceptance. There is no frontend test runner; do not add one in this batch.
- Do not run smoke flags that move the cursor or steal focus without consent.
- Every destructive acceptance run uses an isolated `PPCLIP_DATA_DIR`.
- The code review will run `code-review` (Standards + Spec) plus a UI/design
  review and a requirement-completion matrix.

## Traceability

| Issue | Requirement | Tasks |
|---|---|---|
| L-01 | R-01, R-03 | T1, T2, T3, T5 |
| L-02 | R-02 | T4 |
| L-03 | deferred (needs-info) | — |

## Approval requested

- Approve R-01, R-02, R-03 and the task graph in `tasks.md`, or adjust:
  1. Confirm-only vs undo/trash.
  2. Keep pin cascade vs exempt pins.
  3. Include the About idle-hint disclosure in R-02.
  4. Include the test-only Rust pin-cascade test in R-03.
