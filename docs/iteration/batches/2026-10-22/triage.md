# Triage — 2026-10-22 (local dry-run)

No GitHub labels are applied. State is local only.

| ID | Category | State | Basis |
|---|---|---|---|
| L-01 | `bug` | `ready-for-agent` (local) | Code inspection confirms one-click clear; no confirmation or undo. |
| L-02 | `documentation` | `ready-for-agent` (local) | Code confirms a user-triggered GitHub request; the README's absolute claim is inaccurate. |
| L-03 | `bug` | `needs-info` (local) | Code path suggests a false empty state, but there is no runtime evidence yet. Reproduce before writing an agent brief. |

## Scope direction

- Pilot: **L-01**.
- L-02 is a quick-win candidate but is not part of the pilot.
- L-03 stays `needs-info` until reproduced; it is not selected.

## Verification notes

- **L-01**: `clearAll()` is called directly by the status-bar eraser button. The
  Rust command calls `storage.clear(kind)`; no confirmation gate exists. This is
  a code-confirmed bug/UX risk, not a user-report guess.
- **L-02**: `runCheck()` calls `api.checkLatestRelease()` only when the user
  presses the button. The README's “应用不联网” is an absolute statement that is
  no longer true for that interaction. The fix is wording, not removing the
  feature.
- **L-03**: initial `clips` is `[]`; the placeholder renders whenever
  `clips.length === 0`; the first `refresh()` is async. A false empty state is
  plausible but not yet observed. Do not treat it as confirmed until a runtime
  screenshot/timing check exists.

## Agent brief — L-01

**Category:** bug
**Summary:** Clearing all history is destructive and currently happens on a
single click.

**Current behavior:**
The eraser button in the panel status bar calls `clearAll()` immediately. There
is no confirmation step. `clear_clips` deletes all clips and cascades rows in
`pins`, so pinned desktop windows are removed too.

**Desired behavior:**
The first click opens a confirmation state; nothing is deleted until the user
confirms. Cancel or `Esc` returns to the list with history and pins intact. The
confirmation states the consequence, including that pinned clips are removed.
The dialog/state is keyboard accessible and has a visible focus indicator.

**Key interfaces:**
- Panel clear-all action and its confirmation state.
- Existing `clear_clips` command (no schema change expected).
- Existing pin cascade and viewer close behavior (do not regress).

**Acceptance criteria:**
- [ ] First click on the eraser deletes nothing.
- [ ] Confirmation states that all clips and pinned clips will be removed.
- [ ] Confirm deletes all clips and closes affected viewer/pin windows.
- [ ] Cancel / `Esc` leaves history and pins untouched and returns to the list.
- [ ] Focus is visible; the confirmation is reachable and dismissible by keyboard.
- [ ] No regression to single-clip delete or the empty state.

**Out of scope:**
- Undo/trash/soft delete (separate decision).
- Exempting pinned clips from clear-all (separate product decision).
- Storage or schema changes.

## Questions for the maintainer

1. Is confirmation enough for v1, or is undo required? **Recommended for the
   pilot: confirmation only**, because undo needs a soft-delete model.
2. Should clear-all skip pinned clips, or remove them with a warning? The current
   backend removes them; the UI must say so unless product decides otherwise.
3. Is the one-click behavior intentional? The code has no confirmation and no
   shortcut hint; treat it as a bug until stated otherwise.
