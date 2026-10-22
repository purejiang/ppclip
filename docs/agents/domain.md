# Domain Docs

How the engineering skills should consume this repo's domain documentation.

This is a **single-context** repository: one glossary, one set of design
decisions, no per-package contexts.

## Before exploring, read these

- **`GLOSSARY.md`** at the repo root. Use its terms in issue titles, specs,
  task titles, test names, and review comments. Do not drift to synonyms the
  glossary avoids.
- **`AGENTS.md` → `## 关键设计决策`**. There is no `docs/adr/` yet; these 17
  decisions act as the repo's ADRs, especially for window geometry, docking,
  clipboard self-write suppression, file entries, panel pinning, and the Glacier
  design system.
- **`AGENTS.md` → `## 已知问题`** before proposing a fix that touches a known
  limitation.

If a `docs/adr/` directory appears later, read the ADRs that touch the area you
are about to work in.

## Flag conflicts, don't silently override

If a proposal or implementation contradicts an existing design decision, say so
explicitly:

> _Contradicts AGENTS.md decision 13 (anchor is the geometry source of truth),
> but worth reopening because…_

Do not treat `AGENTS.md` as a suggestion. It documents invariants that were
learned from real bugs.

## File structure

```text
/
├── GLOSSARY.md                  # domain vocabulary
├── AGENTS.md                    # project rules + design decisions + known issues
├── docs/
│   ├── agents/                  # issue tracker / triage labels / this file
│   ├── iteration/               # issue-driven iteration workflow
│   └── adr/                     # optional; create lazily when decisions are recorded
└── src/                         # React frontend
    └── windows/                 # ball / panel / overlay / viewer / pin
```

## When a term or decision is missing

Do not create a glossary entry or ADR speculatively. If a real gap surfaces
during triage, grilling, or review, use the `domain-modeling` skill to record
the term or decision, then update `GLOSSARY.md` and the relevant design
decision. Keep one source of truth: product vocabulary in `GLOSSARY.md`,
invariants in `AGENTS.md` until ADRs are introduced.

## Workflow vocabulary

The issue-driven iteration terms (`batch`, `intake`, `council`, `requirement`,
`task graph`, `acceptance`) are defined in the workflow glossary section of
`GLOSSARY.md`. They are process terms, not product concepts; keep the distinction
when writing specs and issues.
