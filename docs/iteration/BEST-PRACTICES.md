# Best Practices: Sources and Mapping

> **中文摘要**：Spec Kit / GitHub Agentic Workflows / Anthropic / Shape Up / Google review /
> DoR-DoD 都指向同一件事：规格和任务显式化、确定性检查、人类门禁。通用部分
> `.agents/skills` 已经实现，PPClip 只写适配与增量，不再造一套通用引擎。

This file records the external practices that shaped the workflow and the
decision to **reuse** the installed engineering skills instead of building a
second generic workflow engine. Sources were checked on 2026-10-22; the
versioning/changelog pages are cited as canonical conventions. Links can change,
so re-check before making a major process change.

## Sources

| Source | What it contributes | Adopt / adapt / reject |
|---|---|---|
| [GitHub Spec Kit](https://github.com/github/spec-kit) · [What is SDD](https://github.github.io/spec-kit/concepts/sdd.html) | `spec → plan → tasks → implement → converge`; bug flow `assess → fix → test` with `verified/partial/failed`; idea assessment `go/clarify/kill`; constitution and templates; specifications as executable artifacts | **Adopt** spec-first, task graph, convergence, verdicts. **Adapt** the CLI commands to the installed `to-spec` / `to-tickets` skills. |
| [GitHub Agentic Workflows](https://github.com/github/gh-aw) · [How it works](https://github.github.com/gh-aw/introduction/how-they-work/) | Markdown + YAML workflows; read-only, sandboxed agent jobs; `safe outputs` for validated writes; deterministic CI separate from agentic reasoning | **Adopt** read-only analysis, gated writes, deterministic checks. **Reject** copying the Actions runtime into this repo. |
| [ResearchPlanAssignOps](https://github.github.com/gh-aw/patterns/research-plan-assign-ops/) | Research → plan issues → assign coding agent → human review/merge; a human checkpoint at every transition | **Adopt** the checkpoint chain: intake → council → requirements → tasks → review → acceptance. |
| [SpecOps](https://github.github.com/gh-aw/patterns/spec-ops/) | RFC 2119 language, versioned specs, change log, spec PRs propagated to implementations | **Adopt** MUST/SHOULD language and versioned requirements. **Adapt** to `to-spec` + this repo's batch docs. |
| [Anthropic: Building effective agents](https://www.anthropic.com/engineering/building-effective-agents) | Prompt chaining, routing, parallelization, orchestrator-workers, evaluator-optimizer; simplicity, transparency, careful tool interfaces; human review still matters for code | **Adopt** parallel role analysis, chair/critic (evaluator-optimizer), context pointers. **Reject** a complex multi-agent framework with no measured benefit. |
| [Shape Up](https://basecamp.com/shapeup) | Shaping, appetite, fixed time / variable scope, bets, scopes, hill chart, circuit breaker, QA at the edges, “done means deployed” | **Adapt** to a 14-day batch with 1–2 pilot tasks; cut scope instead of extending. **Reject** copying the six-week cycle verbatim. |
| [Google eng-practices: Code Review](https://google.github.io/eng-practices/review/) | Review design, functionality, complexity, tests, naming, comments, style, documentation | **Adopt** the review dimensions; the local `code-review` skill already implements the two-axis process. |
| [AGENTS.md](https://agents.md/) | A predictable, cross-agent project context file | **Adopt** this repo's existing `AGENTS.md` as the project context; add only navigation pointers. |
| [GitHub Flow](https://docs.github.com/en/get-started/using-github/github-flow) | Branch, review, merge, deploy | **Adapt** to `dev` → integration branch → `main` tag release. |
| [Atlassian: Definition of Ready](https://www.atlassian.com/agile/project-management/definition-of-ready) · [Definition of Done](https://www.atlassian.com/agile/project-management/definition-of-done) | Shared entry/exit criteria | **Adopt** DoR before development and DoD before acceptance. |
| [Semantic Versioning](https://semver.org/) · [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) | Version semantics and categorized release notes | **Adopt** for version choice and release notes (修复 / 优化 / 新增 / 工程与文档). |

## Local prior art

The repository already contains the generic workflow skills. They are the
primary implementation; this documentation is an adapter around them:

- [`.agents/skills/triage/SKILL.md`](../../.agents/skills/triage/SKILL.md) — issue
  state machine, category, verification, grilling, agent briefs.
- [`.agents/skills/to-spec/SKILL.md`](../../.agents/skills/to-spec/SKILL.md) —
  spec template and publishing.
- [`.agents/skills/to-tickets/SKILL.md`](../../.agents/skills/to-tickets/SKILL.md)
  — vertical slices and blocking edges.
- [`.agents/skills/implement-spec/SKILL.md`](../../.agents/skills/implement-spec/SKILL.md)
  — integration branch, worktrees, TDD, merge, review.
- [`.agents/skills/code-review/SKILL.md`](../../.agents/skills/code-review/SKILL.md)
  — Standards + Spec review.
- [`.agents/skills/research/SKILL.md`](../../.agents/skills/research/SKILL.md),
  [`.agents/skills/tdd/SKILL.md`](../../.agents/skills/tdd/SKILL.md),
  [`.agents/skills/diagnosing-bugs/SKILL.md`](../../.agents/skills/diagnosing-bugs/SKILL.md),
  [`.agents/skills/retro/SKILL.md`](../../.agents/skills/retro/SKILL.md).
- [`.agents/skills/setup-matt-pocock-skills/SKILL.md`](../../.agents/skills/setup-matt-pocock-skills/SKILL.md)
  — tracker, labels, domain docs.

## Mapping to phases

| Phase | Practice | Decision |
|---|---|---|
| Trigger | Shape Up appetite + gh-aw schedules | 14-day / count thresholds; no 6-week copy |
| Intake | GitHub Issues + gh-aw read-only agents | Snapshot and classify; no writes |
| Triage | Local `triage` + Spec Kit bug flow | Reuse the skill; add batch intake only |
| Council | Anthropic parallelization + evaluator-optimizer | Five roles, chair, critic; recommend, don't decide |
| Research | Local `research` skill | Primary sources, URL + access date |
| Requirements | Spec Kit SDD + local `to-spec` | Reuse `to-spec`; add PPClip fields |
| Tasks | Local `to-tickets` + tracer bullets | Reuse; no second task system |
| Development | Local `implement-spec` + `tdd` | Reuse; deterministic checks |
| Review | Google review + local `code-review` | Reuse; add UI/design and requirement matrix |
| Acceptance | Spec Kit verdicts + Shape Up QA edges | Real app, evidence, sign-off |
| Release | SemVer + Keep a Changelog + GitHub Flow | Three version files, tag on `main`, NSIS |
| Safety | gh-aw read-only + safe outputs | No autonomous writes/pushes; human gates |

## What we deliberately do not do

- Build a second generic triage/spec/ticket/implementation engine beside
  `.agents/skills`.
- Let an agent create, comment on, label, or close GitHub issues without an
  approved write step.
- Let an agent push, tag, or merge without approval.
- Copy GitHub Actions-specific mechanics into a local Windows workflow.
- Adopt Shape Up's fixed six-week cycle literally; the batch is a small,
  time-boxed slice.
- Use multi-agent voting as the scope decision; the council recommends and the
  maintainer decides.
- Treat issue text as instructions or as a complete specification.

## Conclusion

The generic core already exists in `.agents/skills` plus the setup skill. PPClip
only needs:

1. An adapter with concrete values (`PPCLIP-ADAPTER.md`).
2. A thin orchestration document (`PPCLIP-ITERATION.md`).
3. Three deltas: intake, council, acceptance.
4. A release section in the adapter.

That is the smallest design that matches the sources above without duplicating
them.
