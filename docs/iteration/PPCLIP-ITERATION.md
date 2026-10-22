# PPClip Issue-Driven Iteration

> **中文摘要**：这是 PPClip 的薄编排层。0 触发/摄取 → 1 triage → 2 议会 → 3 调研 →
> 4 需求 → 5 任务 → 6 开发 → 7 review → 8 验收 → 9 发布 → 10 复盘。通用阶段直接用
> `.agents/skills` 的现成 skill，不重写；PPClip 只补 `INTAKE.md`、`COUNCIL.md`、
> `ACCEPTANCE.md` 和 adapter 配置。当前 0 issue（2026-10-22），首轮走 bootstrap。
> 每个门禁都要停。

This is the thin orchestration layer for a PPClip release batch. It **does not
redefine** triage, specs, tickets, implementation, or code review. Those rules
live in `.agents/skills/`; this document says which skill owns each phase, what
the phase must produce, and where the human gates are.

## Scope

- Input: GitHub issues in `purejiang/ppclip`.
- Output: one accepted batch (requirements + task graph + code + review +
  acceptance evidence), and optionally a release.
- Not in scope: a parallel generic workflow engine, autonomous pushes/tags, or
  agent-written GitHub issues without approval.
- Current state (2026-10-22): **0 open and 0 closed issues**. The first run is a
  bootstrap (see `INTAKE.md`), not a normal intake.

## Rule: compose, don't duplicate

For every phase, one of these is true:

1. An existing skill owns it; this document only names the skill, the inputs,
   outputs, and the gate.
2. It is a PPClip **delta** with its own file (`INTAKE.md`, `COUNCIL.md`,
   `ACCEPTANCE.md`, or the adapter's release section).

If a phase needs a rule that already exists in `.agents/skills/`, link to it
instead of restating it.

## Preconditions

1. Read `AGENTS.md`, `GLOSSARY.md`, `docs/agents/domain.md`,
   `docs/agents/issue-tracker.md`, and `docs/agents/triage-labels.md`.
2. The tracker config and triage-label mapping are in `docs/agents/`. If the
   repository changes, run `/setup-matt-pocock-skills` again instead of editing
   a copy of its rules.
3. `ppclip.exe` is not running when building, smoke-testing, or capturing UI.
4. Use an isolated `PPCLIP_DATA_DIR` for demos and smoke runs; never write real
   clipboard content into docs or screenshots.

## Phases

| # | Phase | Owner | Input | Output | Gate |
|---|---|---|---|---|---|
| 0 | Trigger + intake | PPClip delta: [`INTAKE.md`](./INTAKE.md) | open issues, last batch state, config | `batches/<id>/intake.md`, ledger update | Batch start (threshold or manual) |
| 1 | Triage | `triage` skill (`.agents/skills/triage/SKILL.md`, or `/triage` if available) | intake snapshot | categorized issues, needs-info questions, agent briefs | Maintainer confirms direction |
| 2 | Council | PPClip delta: [`COUNCIL.md`](./COUNCIL.md) | issues + baseline + evidence | `council.md` (clusters, ambiguities, candidates) | None; analysis only |
| 3 | Research | `research` skill | council clusters | `research.md` with primary sources | None |
| 4 | Requirements | `to-spec` template/skill | council + research + evidence | `requirements.md` with AC, seams, non-goals | **Maintainer approves scope/AC** |
| 5 | Task graph | `to-tickets` rules/skill | requirements | `tasks.md` with `blocked_by`, write scopes | **Maintainer approves the graph** |
| 6 | Development | `implement-spec` + `tdd` / `diagnosing-bugs` | approved tasks | integration branch, tests, before/after screenshots | Local tests pass; no push yet |
| 7 | Review | `code-review` skill + PPClip UI/requirement delta | diff since batch base, requirements, screenshots | `review.md` + fixes | Blocking findings zero |
| 8 | Acceptance | PPClip delta: [`ACCEPTANCE.md`](./ACCEPTANCE.md) | release build, requirements, evidence | `acceptance.md` + verdict | **Maintainer signs off** |
| 9 | Release | Adapter's release section | accepted batch | version bump, release notes, tag, installer | **Maintainer approves push/tag** |
| 10 | Retro | `retro` skill | the batch | retro notes + workflow improvements | None |

### Phase notes

- **0 Trigger + intake**: `triage` is manual. Intake is the missing front door:
  it decides whether a batch is due, snapshots issues, and records
  `unseen / seen / batched`. It does not apply triage labels.
- **1 Triage**: `triage` owns `needs-triage` → `needs-info` /
  `ready-for-agent` / `ready-for-human` / `wontfix`, category labels, grilling,
  and agent briefs. This workflow only batches the work.
- **2 Council**: five independent roles (product, UX, engineering, QA,
  security), then a chair synthesis and a critic. It produces analysis and
  candidate requirements; it does **not** decide scope. Retry a failed role
  once; if it is still missing, record that in `council.md` and continue. Keep
  role outputs compact — the workflow result has a display-size budget.
- **3 Research**: use the `research` skill. Prefer primary sources; record URL
  and access date. Do not paste secondary summaries as evidence.
- **4 Requirements**: use the `to-spec` template (Problem, Solution, User
  Stories, Implementation Decisions, Testing Decisions, Out of Scope, Further
  Notes). Add PPClip fields: source issues, type, evidence, UX/UI, technical
  impact, acceptance criteria, test seams, risk/rollback, non-goals.
- **5 Task graph**: use the `to-tickets` rules: vertical slices, `blocked_by`,
  write scopes, tracer bullets, prefactor first, wide refactors as
  expand–contract. One task must be demoable/verifiable on its own.
- **6 Development**: `implement-spec` owns the integration branch, worktrees,
  implementer/merger subagents, TDD, and closing tickets. This repo adds the
  branch name, commands, and screenshot expectations.
- **7 Review**: `code-review` runs Standards + Spec in parallel. PPClip adds a
  third axis: UI/design compliance and requirement completion.
- **8 Acceptance**: real app, real build, real evidence. Code review is not
  acceptance.
- **9 Release**: three version files, categorized release notes, tag on `main`,
  NSIS workflow. Only after sign-off.
- **10 Retro**: `retro` improves the environment; update this workflow or the
  adapter if the batch exposed a gap.

## Gates

| Gate | When | What must be true |
|---|---|---|
| G0 Batch start | Before intake | Threshold met or maintainer forces a batch |
| G1 Triage direction | After triage | Maintainer agrees with category/state/scope direction |
| G2 Requirements | After `requirements.md` | Problem, scope, AC, seams, non-goals reviewed |
| G3 Task graph | After `tasks.md` | Slices, dependencies, order, write scopes approved |
| G4 Review clean | After fixes | No blocking Standards/Spec/UI/requirement findings |
| G5 Acceptance | After `acceptance.md` | Every selected AC is `verified`; residual risks accepted |
| G6 Release | Before push/tag | Version files match, docs updated, installer builds |

No product code before G2/G3. No GitHub writes before the corresponding
approval. No push/tag before G6.

## Artifacts

Per batch:

```text
docs/iteration/batches/<batch-id>/
├── intake.md          # issue snapshot, new/untriaged, batch decision
├── council.md         # five-role analysis + chair/critic
├── research.md        # primary sources, adopt/adapt/reject
├── requirements.md    # approved spec for the batch
├── tasks.md           # task graph with blocked_by and write scopes
├── review.md          # code + UI + requirement review, dispositions
└── acceptance.md      # real-app evidence and verdict
```

Planned shared files: `issues-state.json` (machine state), `ISSUES.md` (human
ledger), `VERSIONS.md` (release ledger). They are created by the first batch;
they do not exist yet.

`docs/iteration/.runtime/` is for raw issue JSON, temp data, and logs. It is
gitignored; never commit it.

## Quick start

1. Read the preconditions and `INTAKE.md`.
2. Run the trigger/intake check manually (no script yet). If 0 issues, follow
   the bootstrap path in `INTAKE.md`.
3. For each phase, open the owning skill or delta file and produce the listed
   output. Stop at every gate.
4. Before acceptance, read `ACCEPTANCE.md` and prepare an isolated demo data
   directory.
5. Before release, read the adapter's release section and confirm the three
   version files.

## First run (current state)

The repository has no issues. The first run is:

1. Audit the current product: UI screenshots, code paths, logs, known
   limitations, old-version screenshots (`v0.2.0` / `v0.2.5`).
2. Run the council in bootstrap mode to cluster audit findings into candidate
   issues.
3. Present 8–12 candidate issues to the maintainer.
4. Create them on GitHub if `gh` write access is available; otherwise hand over
   paste-ready titles/bodies/labels.
5. Fetch the created issues and continue with the normal phases.

Do not skip the bootstrap and pretend there is an issue backlog.

## Best practices

See [`BEST-PRACTICES.md`](./BEST-PRACTICES.md). The short version: specs and
task graphs are explicit artifacts; the agent reasons, deterministic scripts
verify; writes are gated; review and acceptance are separate; the existing
`.agents/skills` already implement the generic core.
