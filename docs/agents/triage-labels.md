# Triage Labels

The engineering skills speak in terms of five canonical triage roles. This file
maps those roles to the label strings used by `purejiang/ppclip`.

| Canonical role | Label in this tracker | Meaning | Status |
|---|---|---|---|
| `bug` | `bug` | Something is broken | exists |
| `enhancement` | `enhancement` | New feature or improvement | exists |
| `needs-triage` | `needs-triage` | Maintainer needs to evaluate this issue | **to create** |
| `needs-info` | `needs-info` | Waiting on reporter for more information | **to create** |
| `ready-for-agent` | `ready-for-agent` | Fully specified, ready for an AFK agent | **to create** |
| `ready-for-human` | `ready-for-human` | Requires human implementation or decision | **to create** |
| `wontfix` | `wontfix` | Will not be actioned | exists |

Every triaged issue should carry **one category role** (`bug` or `enhancement`)
and **one state role** (`needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, or `wontfix`). If two state labels conflict, ask the
maintainer before changing anything.

Existing labels that remain useful as secondary signals:
`accessibility`, `documentation`, `duplicate`, `question`, `invalid`,
`good first issue`, `help wanted`.

## Creating the missing labels

Run once, from a terminal with `gh` authenticated:

```powershell
gh label create needs-triage --color FBCA04 --description "Maintainer needs to evaluate this issue"
gh label create needs-info --color D876E3 --description "Waiting on reporter for more information"
gh label create ready-for-agent --color 0E8A16 --description "Fully specified, ready for an AFK agent"
gh label create ready-for-human --color 1D76DB --description "Requires human implementation or decision"
```

If `gh` write access is unavailable, record the intended state in the local
intake ledger (`docs/iteration/issues-state.json`, planned) and do **not** apply
a label that does not exist. Do not rename or delete existing repository labels.

## Usage with the skills

- `triage` owns the state machine and the transitions.
- `to-spec` applies `ready-for-agent` after a spec is accepted.
- `to-tickets` publishes child tickets and links them to the spec.
- This repo's iteration docs only add batch intake and the council; they do not
  redefine the triage state machine.

See [`../../.agents/skills/triage/SKILL.md`](../../.agents/skills/triage/SKILL.md)
for the authoritative triage process, and
[`../../.agents/skills/setup-matt-pocock-skills/triage-labels.md`](../../.agents/skills/setup-matt-pocock-skills/triage-labels.md)
for the seed template.
