# PPClip Adapter

> **中文摘要**：这里只放 PPClip 的具体值——repo、标签、分支、构建/测试/验收命令、
> 设计规范、版本文件、发布流程、审批人；triage / spec / tickets / implement / review
> 的规则在 `.agents/skills`，不在这里重复。

This file binds the generic engineering skills to this repository. It contains
**values and pointers only**. The rules for triage, specs, tickets,
implementation, review, and research live in `.agents/skills/`; do not copy them
here.

## Identity

| Field | Value |
|---|---|
| Repository | `purejiang/ppclip` |
| Issue tracker | GitHub Issues; config in [`../agents/issue-tracker.md`](../agents/issue-tracker.md) |
| Triage labels | [`../agents/triage-labels.md`](../agents/triage-labels.md) |
| Domain docs | [`../agents/domain.md`](../agents/domain.md), [`../../GLOSSARY.md`](../../GLOSSARY.md) |
| Project rules | [`../../AGENTS.md`](../../AGENTS.md) |
| Base branch | `dev` for daily work; `main` only receives merges |
| Integration branch | `iteration/vX.Y.Z` created from `dev` (merge `main` in first if `dev` is behind the release) |
| Task branches | one branch/worktree per task, merged into the integration branch |
| Current version | `0.2.5` (as of 2026-10-22) |
| Next version | `0.3.0` by default; `0.2.6` if the batch contains only fixes/docs |

## Issues and batches

- Batch trigger defaults: every **14 days**, or **3** new issues, or **5**
  untriaged issues, or any issue labelled `bug` / `accessibility`.
- Current state (2026-10-22): **0 open and 0 closed issues**. The first run is a
  **bootstrap**: audit the current product, propose candidate issues, get human
  approval, then create them on GitHub or keep the paste-ready list locally.
- Issue intake details: [`INTAKE.md`](./INTAKE.md).
- Multi-role analysis: [`COUNCIL.md`](./COUNCIL.md).
- The canonical triage state machine belongs to
  [`.agents/skills/triage/SKILL.md`](../../.agents/skills/triage/SKILL.md).
  This repo's intake ledger only tracks `unseen / seen / batched`; it does not
  redefine `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`,
  or `wontfix`.

### Environment fallback

- `gh` is the canonical interface. In restricted sandboxes it may fail to launch
  or capture output. For read-only intake, fetch the GitHub REST API with the
  harness's `web_fetch` and save the JSON under `docs/iteration/.runtime/`.
- Creating issues, comments, labels, and closing issues require write access.
  If `gh` cannot write, produce a paste-ready title/body/label block and mark it
  unpublished. Do not invent issue numbers or pretend a write succeeded.
- Never put a token, real clipboard content, or private data into an issue,
  a screenshot, a doc, or a commit.

## Commands

| Purpose | Command |
|---|---|
| Frontend lint | `npm run lint` |
| Frontend build | `npm run build` |
| Rust unit tests | `cd src-tauri; cargo test` |
| Rust type check | `cd src-tauri; cargo check` |
| Release build | `npm run tauri:build` (long; produces the NSIS installer) |
| Fullscreen capture smoke | `.\src-tauri\target\release\ppclip.exe --capture-fullscreen` |
| Region capture smoke | `.\src-tauri\target\release\ppclip.exe --capture-region` |
| Ball wheel smoke | `.\src-tauri\target\release\ppclip.exe --smoke-wheel` |
| Dock smoke | `.\src-tauri\target\release\ppclip.exe --smoke-dock` |
| Panel pin/blur smoke | `.\src-tauri\target\release\ppclip.exe --smoke-panel` |

Preconditions: `dist/` exists; `ppclip.exe` is not running; use an isolated
`PPCLIP_DATA_DIR` for smoke runs so the WebView2 user directory does not collide
with a real instance. `--smoke-dock` moves the real cursor; `--smoke-panel`
steals foreground focus. Run them only with explicit consent.

## Design and standards

- **Invariants**: `AGENTS.md` → `## 关键设计决策` (17 decisions) and
  `## 已知问题`.
- **Design system**: `src/styles/index.css` (Glacier; four font sizes; hidden
  scrollbars; glass cards; aurora tokens).
- **Icons**: `src/components/icons.tsx` only; one icon per meaning; no icon
  fonts; no unused icons.
- **UI surfaces**: `src/windows/` (`BallApp`, `PanelApp`, `OverlayApp`,
  `PinApp`, `ViewerApp`) and `src/components/` (`ClipRow`, `SettingsPage`,
  `AboutPage`).
- **No new UI/state/i18n/icon libraries** without explicit approval.
- **Design review** is an extra review axis, not a replacement for
  `code-review`: screenshots before/after, state coverage, focus visibility,
  contrast, target size, reduced motion, and the Glacier rules.

## Acceptance

Real acceptance is app-level, not just green tests. Use
[`ACCEPTANCE.md`](./ACCEPTANCE.md).

- Build the release executable, run with an isolated demo data directory, and
  verify each acceptance criterion against real windows.
- Required automated checks: `npm run lint`, `npm run build`, `cargo test`,
  `cargo check`, plus the relevant smoke flags.
- Required manual/visual checks: 100% and 150% DPI, multi-monitor selection,
  restart persistence, missing files, permission-denied paths, fullscreen
  exclusive apps.
- Evidence: screenshots with a manifest (version, commit, DPI, window rect,
  data directory, capture method). Do not use real clipboard data.
- Verdict per requirement: `verified` / `partial` / `failed`. No release with a
  `failed` or unresolved `partial`.

## Release

- Version must match in three files:
  `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
- Patch (`0.2.x`) if the batch contains only fixes/docs; minor (`0.3.0`) if it
  adds a feature or changes behavior.
- Release notes are categorized: 修复 / 优化 / 新增 / 工程与文档.
- Tag `vX.Y.Z` is created on `main`; `.github/workflows/release.yml` builds the
  NSIS installer and creates the GitHub Release.
- Closing/commenting on issues after release is a separate, approved write; do
  not close issues automatically.

## Human approvals

| Gate | Approver | What is approved |
|---|---|---|
| Bootstrap candidate issues | maintainer | issue titles/bodies/labels before creation |
| Triage direction | maintainer | category/state/scope direction |
| Requirements | maintainer | problem, scope, acceptance criteria |
| Task graph | maintainer | slices, dependencies, order |
| Push / tag | maintainer | integration branch push, tag, release |
| Acceptance | maintainer | real-app evidence and final sign-off |

Batch **start** is a human decision today; the thresholds are the recommended
trigger. The intake script is planned, so do not assume scheduling exists. No
GitHub writes or product changes happen before the corresponding gate.

## Artifact layout (planned)

The first batch creates these; they are not present yet:

```text
docs/iteration/
├── PPCLIP-ITERATION.md      # thin orchestration (this workflow)
├── PPCLIP-ADAPTER.md        # this file
├── INTAKE.md                # trigger / snapshot / bootstrap
├── COUNCIL.md               # multi-role analysis
├── ACCEPTANCE.md            # real acceptance
├── BEST-PRACTICES.md        # sources and mapping
├── issues-state.json        # machine ledger (planned)
├── ISSUES.md                # human issue ledger (planned)
├── VERSIONS.md              # release ledger (planned)
├── batches/<batch-id>/      # intake / council / research / requirements / tasks / review / acceptance
├── evidence/<version>/      # screenshots + manifest
└── .runtime/                # raw issue JSON, temp data; gitignored
```

## What not to duplicate

Do not add a second copy of triage, spec, ticket, implementation, or code-review
rules to this repo. Use:

- `triage` for the state machine and agent briefs.
- `to-spec` for the spec template and publishing.
- `to-tickets` for vertical slices and blocking edges.
- `implement-spec` for the integration branch and subagent workflow.
- `code-review` for Standards + Spec review.
- `research` for primary-source research.
- `tdd` / `diagnosing-bugs` for test-first development and hard bugs.
- `retro` for the retrospective.

This adapter and `INTAKE.md` / `COUNCIL.md` / `ACCEPTANCE.md` are the only
PPClip-specific additions.
