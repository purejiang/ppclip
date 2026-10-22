# Retro — 2026-10-22 (local dry-run)

This is a **process retro** for the dry run, not a release retro. The batch ran
through intake, triage, council, research, requirements, and task breakdown; no
product code was written.

## What worked

- Three candidates were enough to exercise the flow: one FIX, one docs FIX, one
  needs-info UI hypothesis.
- The council produced a clear pilot recommendation: clear-all confirmation
  (C-01) plus acceptance evidence (C-04); L-03 stayed `needs-info`.
- Research narrowed L-03: the ordinary first open usually mounts hidden long
  before reveal; the reachable false-empty path is pinned startup, plus two
  timing-independent sub-states (`0 / 300` while stats is null, and the
  placeholder beside an error). This is exactly the kind of correction triage
  alone could not make.
- The critic caught a real self-contradiction (C-03 shipping a frontend slice
  while L-03 was deferred) and a scope leak in C-04 (L-03 evidence inside an
  L-01 requirement). Both were fixed in `council.md` / `requirements.md`.
- The task graph is small and has genuine blockers: T1 → T2 → T5, with T3/T4
  parallel because their write scopes do not overlap.

## What failed or was missing

1. **Full council output was truncated.** The first workflow returned ~166k
   characters; the job-output display cut at ~50k and the raw JSON was not
   parseable. The compact retry fit. Council prompts/schemas need an output
   budget from the start.
2. **One role failed and aborted the run.** The first compact run failed because
   QA returned no result. The retry/fallback version completed. A missing role
   must be recorded, not fatal.
3. **`web_search` was unavailable** (HTTP 401). Research used direct `web_fetch`
   on primary sources; the workflow needs this fallback documented.
4. **Local dry-run mode was not documented.** `INTAKE.md` assumes GitHub issues;
   this run used local IDs (`L-01`…`L-03`) and did not write the shared ledger.
   It was improvised.
5. **No scripts exist yet.** Intake was manual; there is no scheduler, no
   `issue-intake.ps1`, no `acceptance.ps1`. The dry run confirms the docs are the
   right next artifact, but automation is still pending.
6. **No runtime evidence for L-03.** The record should stay `needs-info`; the
   dry run did not launch the app or capture the pinned-startup frame.
7. **Triage labels are not created.** The dry run used local state only; the
   GitHub labels from `triage-labels.md` still need creation when `gh` write
   access is available.

## Workflow changes to make now

- `INTAKE.md`: add a **local dry-run mode** section (local IDs, batch-local
  records, no shared ledger, no GitHub writes).
- `COUNCIL.md`: add **operational notes** — compact role outputs, retry a failed
  role once, mark a still-missing role, never let one role abort the council,
  and budget the final output size.
- `PPCLIP-ITERATION.md`: add a phase-2 note that role failure is non-fatal and
  that a missing perspective must be recorded in `council.md`.
- Keep the `research` fallback explicit: direct `web_fetch` when `web_search` is
  unavailable; record failed URLs instead of silently replacing them.

## What to do next

1. Maintainer approval of `requirements.md` and `tasks.md` (the two gates).
2. Implement T1/T2/T3 (pilot) on `iteration/v0.3.0`, then T4 (docs), then T5
   (acceptance). Do not push/tag without approval.
3. Run `code-review` (Standards + Spec), a UI/design review, and the
   requirement-completion matrix. Fix blocking findings.
4. Build the release exe, run the isolated-data acceptance runbook, and record
   the verdict and sign-off.
5. After acceptance, decide whether to release; if yes, bump the three version
   files, write release notes, tag on `main`, and update `VERSIONS.md`.
6. Add the automation pieces later: `issue-intake.ps1`, `acceptance.ps1`, and a
   scheduled trigger. The dry run gives their acceptance tests: threshold
   decisions, bootstrap, role retry, output budget, and isolated-data acceptance.

## Metrics from this dry run

- Candidates: 3 (1 FIX, 1 docs FIX, 1 needs-info).
- Council roles: 5 + chair + critic.
- Selected requirements: 3 (R-01 pilot, R-02 docs quick win, R-03 evidence).
- Deferred: L-03, undo/trash, pin-exemption policy.
- Tasks: 5 (T1/T2 critical path, T3/T4 parallel, T5 acceptance).
- Product code changed: 0.
- GitHub writes: 0.
