# Council — 2026-10-22 (local dry-run)

- **Mode**: local dry-run; no GitHub issues; no product code.
- **Inputs**: `intake.md`, `triage.md`, `research.md`, `AGENTS.md`, `GLOSSARY.md`.
- **Roles**: product, UX, engineering, QA, security; chair synthesis + critic.
- **Coverage**: all five roles returned. The first compact run aborted when the QA
  role returned no result; a retry/fallback run completed. This is recorded in
  `retro.md` as an operational finding.

## Clusters

| ID | Issues | Theme | Why |
|---|---|---|---|
| K-1 | L-01 | Destructive-action safety: clear-all on the panel status bar | One click on the 14px eraser destroys every clip, cascades pins (closing pin windows and the viewer), and unlinks image/thumbnail files. Single caller in `PanelApp.tsx`, so frontend-only, but the only data-loss path in the batch. |
| K-2 | L-02 | Privacy claim accuracy: the offline promise vs the one update request | Four absolute sentences plus the About page copy contradict one user-triggered GitHub Releases request. Docs/copy only, separable from the pilot. |
| K-3 | L-03 | Unknown vs empty: first-paint states in the panel | Loading, failed read, and genuine emptiness collapse into one placeholder; the status bar prints `0 / 300` before stats land. The only timing path is pinned startup, and there is no frame yet. |

## Issue readings

### L-01 — clear-all

**Problem.** The status-bar eraser fires `clearAll()` on the first click
(`src/windows/PanelApp.tsx:457-466` → `:285-295`, sole caller `:288`).
`clear_clips` deletes every clip, cascades pins (`src-tauri/src/storage.rs:35`,
`ON DELETE CASCADE` with foreign keys on), closes pin windows and the viewer
(`src-tauri/src/commands.rs:429-450`), and unlinks image/thumbnail files
(`storage.rs:335-338`). No confirmation, no undo.

**Expected.** First click only arms a confirmation; only the explicit confirm
calls `clear_clips` unchanged. Copy names the real blast radius: N clips, pins
closed with their pin windows, viewer closed, image/thumbnail files removed from
the data dir, file-clip source files untouched, cannot be undone, not a secure
wipe. Outcome-labelled buttons, safe initial focus, Esc cancels at its own layer,
Enter/Arrow inert, focus returns, real count announced via `role="status"`, no
auto-revert timer.

**Ambiguities.** Count source while stats is null/stale (`stats?.total` can be 0
or change before confirm); `pinnedIds` comes from `listPins().catch(() => [])`, so
a failed fetch can look like zero pins. Placement: `.panel-status` is fixed
32px (`src/styles/index.css:742-747`), so a consequence sentence cannot live
inside it unchanged. Hidden-not-unmounted panel means an armed confirmation can
survive into the next summon with a stale count.

### L-02 — offline claim

**Problem.** Four absolute sentences (`README.md:34`, `:112`;
`README.en.md:32`, `:110`) plus the in-app “离线可用” line
(`src/components/AboutPage.tsx:229`) versus one user-triggered WinHTTP GET to a
fixed host (`src-tauri/src/github.rs:23-26`), reached only from 检查更新
(`AboutPage.tsx:52-68`). No clip content is sent; nothing is downloaded or
installed.

**Expected.** One scoped statement in both languages and in-app: data stays
local and is never uploaded or synced; offline by default; exactly one GitHub
Releases API request only when the user presses 检查更新; the trigger is manual;
nothing downloaded or installed; a failed check affects no local feature
(known-limitation line). Feature unchanged.

**Ambiguities.** Scope: the two intake lines vs all four README sentences vs the
in-app About line. Whether the About idle hint should disclose the request
before the click.

### L-03 — false empty state

**Problem.** `clips` starts `[]` (`PanelApp.tsx:45`) and the placeholder keys on
`clips.length === 0` (`:406-411`) while `refresh` is async (`:114-135`), so
loading, failed read, and genuine emptiness all render “还没有记录”;
`stats?.total ?? 0` prints `0 / 300` before stats land (`:435`). Reachable timing
path is `panel_pinned` startup (`src-tauri/src/lib.rs:1130-1136`), which shows
the panel during setup.

**Expected.** Unknown is a third state: never “还没有记录” before a successful
list; error owns the list area and never sits beside the empty copy; “没有匹配
的记录” only for a query that genuinely returned zero, scoped per pill filter;
no count and a disabled eraser while stats are unknown; non-focus results
announced via `role="status"`.

**Ambiguities.** No reproduction yet; whether `panel_pinned` at launch is a real
configuration. Which surfaces count: list placeholder only, or also the `0 / 300`
count and the footer “Ready”.

## Candidate requirements

| ID | Type | Source | Title | Proposed direction |
|---|---|---|---|---|
| C-01 | FIX | L-01 | Gate clear-all behind a confirmation that names what it destroys | First click only arms; explicit confirm calls the unchanged `clear_clips`. Copy states counts and pin/viewer/image-file consequences; outcome-labelled buttons; safe initial focus; Esc layer; focus return; `role="status"`; no timer; disable while stats unknown. |
| C-02 | FIX | L-02 | Scope the offline claim to the single manual update check | Reword all four README sentences + known-limitation line + About copy: local data, offline by default, one manual GitHub request, nothing uploaded/downloaded. Keep the feature. |
| C-03 | FIX | L-01, L-03 | Model unknown, error, and empty as distinct states | `loaded` flag; never “还没有记录” before a successful list; error owns the list; no fake count; disable eraser while stats unknown. |
| C-04 | ADD | L-01 | Acceptance evidence for the destructive path | Isolated `PPCLIP_DATA_DIR` runbook + missing Rust test for `clear()` pin cascade; no frontend test framework. |

## Conflicts with existing decisions

- **Decision 11** (row delete is a two-step with a 3s auto-revert): C-01
  deliberately does not reuse the timer. An auto-reverting state is a time limit
  (WCAG 2.2.1). The spec must state which mechanism applies where.
- **Decision 14** (Esc chain: About → settings → hide panel): C-01 inserts a
  layer, so the footer hint “Esc 关闭” is momentarily inaccurate. It must change
  to “Esc 取消” while armed.
- **Decisions 5 / 16** (eviction protects pins; clear-all cascades them): the
  copy must not imply pins survive or data is recoverable, and must not read as
  deleting the user’s source files.
- **Decision 15** (clear-all lives on the status bar, not settings): the
  confirmation stays in-panel, but `clear_clips` remains an ungated command for
  future callers. This is a recorded risk, not a blocker.
- **Decision 17** (panel hidden, not unmounted): an armed confirmation survives
  a hide unless it is reset. C-01 needs an explicit reset rule.
- **Decision 14 / release section**: no updater, 检查更新 only detects. C-02 must
  not imply automatic download.
- **Research step 12** (“no modal”) is a preference, not a house rule. The panel
  already ships AboutPage as `role="dialog"`/`aria-modal` without a focus trap;
  choosing a modal makes the APG trap mandatory.

## Evidence gaps

- L-03 has no reproduction. Needs `PPCLIP_DATA_DIR` + `panel_pinned: true` and a
  frame capture in roughly the first 200–500 ms, or `frontend_log` timestamps
  bracketing the first refresh. Until then it stays `needs-info`.
- No frontend test runner (no test script, no `src/**/*.test.*`), and
  `--smoke-panel` asserts window geometry only, so focus/Esc/`role="status"` are
  manual evidence.
- The pin cascade is untested: `storage.rs:924-943` asserts kinds and image files,
  not pins.
- Contrast and motion unmeasured: `--error #ff6b6b`, the danger button, and any
  new focus ring have no contrast calculation; `index.css` has one
  `:focus-visible` rule and zero `prefers-reduced-motion`.
- Screen-reader behaviour (NVDA/Narrator) and the 2px + 3:1 focus target are
  unverified. The undo-vs-confirm authority is still missing (NN/g undo URL 404).
- Placement has no design: `.panel-status` is fixed 32px; an inline confirmation
  shrinks the clip list by a line.

## Recommended batch (non-binding)

**Theme.** One pilot: clear-all gets a confirmation that names what it destroys,
backed by isolated acceptance evidence. The docs-only privacy fix rides along
separately and must never gate the pilot. L-03 waits for a reproduction.

- **Selected**: C-01, C-04.
- **Deferred**: C-02, C-03.
- **Non-goals**: undo/trash/soft delete; changing whether clear-all cascades
  pins; removing or automating 检查更新; a frontend test framework; Rust command,
  schema, or migration changes; writing L-03 before a reproduction; global
  focus-ring / reduced-motion retrofit beyond what the confirmation needs.

## Critic findings and dispositions

| Target | Verdict | Reason | Disposition |
|---|---|---|---|
| Chair summary / recommended batch | keep | Faithful to code; one pilot + L-03 held is sound. Product undercounted L-02 as two sentences; four exist. | Keep; C-02 covers four. |
| C-01 copy scope, count source, placement | revise | Placement never decided; `M pins` is unsafe because `pinnedIds` comes from a falling-back `listPins`; secure-wipe/source-file clauses are policy, not settled. | Revise: choose an inline strip above the status bar; show clip count, and pin count only when the pin list loaded; use indeterminate wording otherwise; keep source-file/secure-wipe clauses out of the required copy unless the maintainer wants them. |
| C-01 Esc layer / hide persistence / decisions 15,17 | revise | Decision 15 only fixes location; it does not say “confirmation stays in-panel”. Panel hidden-not-unmounted means a stale armed state can survive. Footer hint unanswered. | Revise: add reset-on-hide, footer hint “Esc 取消”, and do not cite decision 15 as placement support. |
| C-03 | revise | Self-contradiction: ships a frontend slice while L-03 is deferred; a deferred candidate invites unreproduced work. | Drop C-03 as a separate requirement. Fold the stats-unknown/disable slice into C-01. Keep L-03 as `needs-info`. |
| C-04 | revise | Source says L-01 only, but body includes L-03 frame capture and `frontend_log`; also adds a Rust test while K-1 says frontend-only. | Revise: keep isolated-data runbook + Rust pin-cascade test; move L-03 evidence out; label the Rust change test-only. |
| README.md:38 drift claim | drop | README:38 only lists the feature, no one-click wording; no drift. | Drop. |
| Chair questions | revise | Too many; some ask the maintainer to design for a caller that does not exist. | Keep at most two per issue; demote “gate in Rust” to a recorded risk. |
| C-02 content, deferral | revise | Content verified and cheap; a false absolute claim ships today. Docs-only work cannot gate a code pilot, so “deferred” means knowingly leaving it live. | Sequence the docs edit **after C-01** in the same batch; do not drop it. |

## Lead decisions for the requirements draft

These are proposed, pending maintainer approval:

1. **Confirm-only for v1**, no undo/trash. Undo needs a soft-delete model; it is
   a separate product decision.
2. **Keep the current pin cascade** (no Rust behavior change). The confirmation
   copy states that pinned windows are closed.
3. **Inline confirmation strip above the status bar**, not a modal. The strip
   expands the status area; the list shrinks. The safe action `取消` occupies the
   eraser's old far-right position so a double-click cannot confirm; the
   destructive `清空 N 条` is offset left. Focus starts on `取消`.
4. **Counts are best-effort.** Show `stats.total` for clips; show pin count only
   if the pin list loaded; otherwise use indeterminate wording, never `0`.
5. **Reset on hide.** If the panel hides while armed, drop the confirmation; the
   next summon starts unarmed.
6. **C-02 is included as a docs/copy quick win after C-01**, not a pilot gate:
   four README sentences + known-limitation line + About idle-hint disclosure.
7. **C-03 is not a requirement.** Fold only “stats unknown → no fake 0, eraser
   disabled” into C-01; L-03 stays `needs-info` with the narrowed reproduction.

## Open questions for the maintainer

1. **L-01**: confirm-only for v1 (recommended), or do you want undo/trash in
   scope? Undo is a larger soft-delete change.
2. **L-01**: keep the current cascade (clear-all also clears pins) and state it
   in the copy, or exempt pinned clips? Exempting is a separate product/backend
   decision.
3. **L-02**: include the About idle-hint disclosure, or docs-only? The
   recommendation is to include it so consent happens at the click.
4. **L-03**: do you want to reproduce the pinned-startup frame before writing a
   requirement, or accept the “unknown/error/empty” model without a reproduced
   flash?

## Dry-run observations

- The full council result exceeded the job-output display limit (~50k chars) and
  was truncated; the compact rerun fit. Workflow output must be budgeted.
- One compact run failed because the QA role returned no result; retry/fallback
  per role is required. A missing role should not abort the whole council.
- `web_search` was unavailable (HTTP 401); research used direct `web_fetch` on
  primary sources. The workflow needs a documented fallback.
- The local dry-run mode (local IDs, no GitHub writes) was not in `INTAKE.md`;
  it was improvised. The doc should add it.
