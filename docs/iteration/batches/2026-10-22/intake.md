# Intake — 2026-10-22 (local dry-run)

- **Mode**: local dry-run. No GitHub issues are created, commented on, labelled,
  or closed.
- **Trigger**: `BOOTSTRAP_REQUIRED` — GitHub has **0 open and 0 closed issues**
  (checked 2026-10-22). This batch is a manual **A-test** of the workflow, not a
  real release batch.
- **Batch id**: `2026-10-22`.
- **Inputs**: three candidate issues extracted from the current codebase. They
  are not on GitHub yet; IDs are local (`L-01`…`L-03`).

## Candidates

| ID | Title | Type | Labels | Evidence |
|---|---|---|---|---|
| L-01 | Clear all history takes effect on a single click | FIX | `bug` | `src/windows/PanelApp.tsx:285-295`, `:457-466`; `src-tauri/src/commands.rs:418-426` |
| L-02 | README says the app makes no network requests, but About can call GitHub | FIX / DOCS | `documentation` | `README.md:34`, `README.en.md:32`; `src/components/AboutPage.tsx:52-68`; `src-tauri/src/github.rs` |
| L-03 | Panel may show the empty state before the first list request returns | FIX / UI | `bug` | `src/windows/PanelApp.tsx:45-46`, `:405-411`, `:155-158` |

## Why these three

- **L-01**: real data-loss risk, small UI-only blast radius, testable — the
  intended pilot candidate.
- **L-02**: trust/docs inconsistency, cheap to fix — could be a quick win.
- **L-03**: exercises the non-destructive UI path and the “needs reproduction”
  triage state.

## Questions

- **L-01**: confirmation vs undo? Does clear-all include pinned clips (the
  backend currently cascades pins)? What copy and warning are required?
- **L-02**: exact wording: “默认不联网；仅在你点击检查更新时请求 GitHub”?
- **L-03**: is the false empty state observable on this machine? Needs runtime
  evidence before it is ready-for-agent.

## Suggested clusters (non-binding)

- Destructive action safety: L-01
- Trust / documentation: L-02
- First-paint / loading state: L-03

## Ledger note

The shared `issues-state.json` / `ISSUES.md` ledgers are for real GitHub issues
and are not written in this local dry-run. The batch-local `intake.md` is the
input of record.

## Gate

Maintainer approves the candidate set and the pilot selection before
requirements/tasks are frozen.
