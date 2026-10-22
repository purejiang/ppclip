# Tasks — 2026-10-22 (local dry-run)

- **Status**: implemented on `iteration/v0.3.0` (product commit `fb25df6`).
  Acceptance: `acceptance.md`; review: `review.md`. T1–T5 are complete; three
  R-01 checks (failure injection, manual screen reader, 150% DPI) remain partial
  and are listed in `acceptance.md`.
- **Batch**: `2026-10-22`.
- **Requirements**: `requirements.md` (R-01, R-02, R-03).
- **Integration branch** (when implementation starts): `iteration/v0.3.0` from
  `dev`; local commits only, no push/tag without approval.

## Implementation status

| ID | Status | Evidence |
|---|---|---|
| T1 | done | `137e043`; `02-confirm-armed.png`, `03b-double-click-safe.png` |
| T2 | done; failure path partial | `137e043`, `fb25df6`; UIA focus/Tab/Esc/Enter/reset/success checks; failure injection deferred |
| T3 | done | `171701e`; `cargo test` 68/68 |
| T4 | done | `c1b319d`; READMEs + `08-about-copy.png` |
| T5 | done with residuals | `acceptance.md`, `evidence/`; three partial checks called out |

The checkboxes in the task sections below are the original acceptance
criteria; see `acceptance.md` for the verified/partial disposition. Marking a
checkbox unchecked here does not mean the task was not attempted.

## Task graph

| Order | ID | Requirement | Title | Blocked by | Write scopes |
|---|---|---|---|---|---|
| 1 | T1 | R-01 | Clear-all confirmation state, strip UI, copy, safe layout | — | `src/windows/PanelApp.tsx`, `src/styles/index.css` |
| 2 | T2 | R-01 | Keyboard/focus/Esc/status announcement/error/reset-on-hide | T1 | `src/windows/PanelApp.tsx`, `src/styles/index.css` |
| 2 | T3 | R-01, R-03 | Rust regression test for `clear()` pin cascade | — | `src-tauri/src/storage.rs` (tests module only) |
| 3 | T4 | R-02 | Scope the offline claim in READMEs and About copy | — | `README.md`, `README.en.md`, `src/components/AboutPage.tsx` |
| 4 | T5 | R-03 | Acceptance runbook, isolated data run, evidence | T1, T2, T3, T4 | `docs/iteration/batches/2026-10-22/acceptance.md`, `docs/iteration/batches/2026-10-22/evidence/` |

T1 → T2 is the critical path. T3 and T4 can run in parallel with T1/T2 because
their write scopes do not overlap; T4 is ordered after T1 only to keep the batch
focused, it has no technical blocker. T5 runs after all code/copy changes.

## T1 — confirmation state and strip

**Delivers:** First click on the eraser arms a confirmation and deletes nothing.
The strip appears above the status bar with the consequence copy and
`取消` / `清空 N 条`; the safe action is the far-right action of the strip, and
the eraser's old coordinates are covered by the cancel-on-click backdrop. The
destructive action is offset left. The list/header are inert while armed.

**Acceptance criteria:**
- [ ] First click does not call `api.clearClips`; no DB/file/pin change.
- [ ] Strip shows clip count when `stats` is loaded; indeterminate wording when
      stats or the pin list is unavailable; never a false `0`.
- [ ] `取消` is at the far right (the eraser's old position); `清空 N 条` is left
      of it.
- [ ] Eraser is disabled when `stats` is unknown or `stats.total === 0`.
- [ ] Copy states: pins close with their windows, image/thumbnail files are
      deleted, file-clip source files are untouched, cannot be undone.
- [ ] Rest of the panel cannot be activated while armed (inert or equivalent).
- [ ] `Esc` / outside click / `取消` cancel without data change.
- [ ] Strip fits 420×600 without pushing the status bar or footer off-screen;
      no new font size; danger styling uses existing tokens.

**Verification:** manual/UIA + screenshot; no API call observed; no DB/file
changes in an isolated data directory.

## T2 — keyboard, focus, announcement, error, reset

**Delivers:** The confirmation is fully keyboard accessible and does not leak
into the panel's other key handlers. Success is announced; failure is recoverable;
hiding the panel resets the armed state.

**Acceptance criteria:**
- [ ] Initial focus is `取消`; focus is visible (2px, ≥3:1 change); Tab cycles
      only between `取消` and `清空 N 条`.
- [ ] `Enter` activates only the focused button; it never confirms globally.
- [ ] `Esc` order: confirmation → About → settings → hide panel.
- [ ] Footer reads `Esc 取消` while armed, `Esc 关闭` otherwise.
- [ ] Success: strip closes, list refreshes, `role="status"` announces
      `已清空 N 条` (N = `clearClips` return), focus moves to search (eraser is
      disabled).
- [ ] Failure: strip remains open, error is visible, buttons re-enable, no data
      changed.
- [ ] Hiding the panel while armed resets the confirmation; reopening starts
      unarmed with no stale count.
- [ ] New transition respects `prefers-reduced-motion`.

**Verification:** keyboard-only UIA/manual pass; screenshot of focus ring;
failure injection (read-only data dir or temporary error); hide/reopen.

## T3 — Rust pin-cascade regression test

**Delivers:** A test that proves `clear(None)` removes clips **and** the pins
that reference them, and removes image/thumbnail files. Test-only; no production
behavior change.

**Acceptance criteria:**
- [ ] Test creates a clip, a pin, and image/thumb files, calls `clear(None)`.
- [ ] Asserts clips and pins are gone; image/thumb files are gone.
- [ ] `cargo test` passes.
- [ ] No production code change beyond the test module.

**Verification:** `cd src-tauri; cargo test`.

## T4 — scoped offline claim

**Delivers:** The four README sentences and the About page copy describe one
manual GitHub request accurately, in both languages.

**Acceptance criteria:**
- [ ] `README.md` and `README.en.md` no longer contain absolute “no network”
      claims; both say: local data, offline by default, one GitHub Releases
      request only on 检查更新, no clipboard content sent, nothing downloaded or
      installed.
- [ ] Known-limitation line added: check-update needs network; failure affects no
      local feature.
- [ ] About idle hint discloses the request before the click.
- [ ] About “离线可用” line is aligned with the scoped claim.
- [ ] Feature unchanged: check can run, fail gracefully, open the Release page.
- [ ] Chinese and English are sentence-for-sentence parallel.

**Verification:** text review; screenshot of the About idle hint; one real check
(success or graceful failure).

## T5 — acceptance evidence

**Delivers:** A repeatable acceptance runbook and evidence for R-01/R-02, using
an isolated `PPCLIP_DATA_DIR` and the real release app.

**Acceptance criteria:**
- [ ] `acceptance.md` contains the environment, steps, expected/actual, evidence,
      and `verified` / `partial` / `failed` verdicts.
- [ ] No real clipboard/history is used; the default data directory is untouched.
- [ ] Covers: first click no-op; cancel/Esc; confirm; pin windows closed; viewer
      closed; images/thumbs deleted; empty state; keyboard/focus; hidden reset;
      error path; About hint.
- [ ] Screenshots + commands/exit codes attached.
- [ ] Maintainer signs the acceptance record.

**Verification:** real release exe; isolated data dir; UIA/manual; screenshots.

## Definition of Ready (batch)

- [x] Source candidates and evidence are recorded.
- [x] Council and research are recorded.
- [x] R-01/R-02/R-03 have testable acceptance criteria.
- [x] Task graph and write scopes are explicit.
- [x] Maintainer approves the requirements and this graph.
- [x] Open questions (confirm-only, pin cascade, About hint, Rust test) are
      answered.

## Definition of Done (per task)

- Acceptance criteria checked with evidence.
- Relevant automated checks pass (`cargo test`, `npm run lint`, `npm run build`,
  `cargo check`).
- No regression in the touched area.
- UI changes have before/after screenshots.
- Code review (Standards + Spec) and UI/requirement review have no blocking
  findings.
- Docs/version notes updated if behavior or claims changed.
- Task status updated in this file.

## Out of scope for this batch

- Undo/trash/soft delete.
- Pin-exemption policy.
- L-03 loading/error/empty requirement (needs reproduction).
- A frontend test framework.
- Rust command/schema/migration changes.
- Global focus-ring/reduced-motion retrofit.
- Push, tag, release, or issue closure.
