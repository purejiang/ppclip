# Review — 2026-10-22 (local dry-run)

- **Status**: review complete; all blocking findings fixed and re-tested.
  Maintainer signed off on 2026-10-22 with residuals; release decision pending.
- **Batch**: `2026-10-22`.
- **Requirements**: `requirements.md`.
- **Branch**: `iteration/v0.3.0`.
- **Product diff**: `9a438f3...fb25df6` (the workflow-docs commit `9a438f3` is
  the approved iteration-workflow base, not part of R-01–R-03).
- **Acceptance**: `acceptance.md` and `evidence/`.

## 中文摘要

按 `code-review` 的两轴跑了一遍：Standards 找到 3 个硬问题（确认条不跟随
面板不透明度、错误注释、`.agents` 本地 skill 引用未说明），已修；Spec 找到
3 个功能/文案缺口（About 少「不自动下载」、`role=status` 条件挂载、中英文
措辞不完全对齐），已修；「取消位置」按安全结果做了实现偏差记录。Fowler
层面的重复 switch / 重复 rAF 聚焦已合并，组件拆分和数据合并作为非阻塞项
保留。真实 UIA 验收又抓到一个 Tab 陷阱条件写反的 bug（Tab 从取消跳到了
状态条），已修并重跑通过。UI 按截图复核：确认条内联、取消在右、危险按钮
在左、焦点环可见、50% 不透明度可读。当前无阻塞项；失败注入、读屏人工、
150% DPI 仍是 residual risk。

## Fixed point and scope

| Item | Value |
|---|---|
| `dev` base | `866be06` |
| Workflow docs | `9a438f3` — approved in the previous turn; not R-01–R-03 scope |
| Product implementation | `137e043` (T1/T2), `171701e` (T3), `c1b319d` (T4), `fb25df6` (review fixes + Tab trap) |
| Spec | `requirements.md` R-01/R-02/R-03 |
| Standards | `AGENTS.md` design decisions, `README.md`, `.oxlintrc.json`, existing style |

The first review pass ran against `866be06...c1b319d`; the re-test pass ran
against the final product tree at `fb25df6`.

## Standards review

### Hard findings

| # | Finding | Disposition |
|---|---|---|
| S1 | `.panel-clear-confirm` used fixed `96%`/`30%` alpha and bypassed the panel-opacity slider (`AGENTS.md` decision 14) | **Fixed**: border/background now use `calc(... * var(--panel-opacity))`; 50% screenshot `07-confirm-opacity-50.png` |
| S2 | Touched comment claimed blur-hide is `WindowEvent::Focused`; decision 17 says it is foreground polling (`lib::watch_panel_blur`) | **Fixed** in the touched comment |
| S3 | `AGENTS.md`/README referenced `.agents/skills/...`, but `.agents/` was untracked, so links break in a clone | **Mitigated**: `.agents/`, `.claude/`, `skills-lock.json` are now gitignored as local skill installation; `AGENTS.md`, `PPCLIP-ITERATION.md`, and `BEST-PRACTICES.md` state that the pack is local and how to proceed without it. Not vendored into the repo. |

### Judgement calls

| Smell | Disposition |
|---|---|
| Repeated switches for clear summary | **Fixed**: bundled into one `clearSummary` object (`countText`, `pinText`, `confirmLabel`, `eraserDisabled`, `eraserTitle`) |
| Duplicated focus-after-paint (3×) | **Fixed**: module-level `focusAfterPaint()` helper |
| Data clumps (`pinsLoaded` + `pinnedIds`) | **Kept intentionally**: `pinnedIds` drives row toggle state, `pinsLoaded` drives the indeterminate copy; merging them would tie two concerns together |
| Divergent change: PanelApp grew ~220 lines | **Accepted for pilot**: the strip is cohesive and lives next to its state; extract `ClearConfirmStrip` if it grows or gets reused |
| Duplicated focus-ring CSS | **Scoped**: one rule for the confirmation buttons and status buttons; not merged with the settings-card rule (different selectors/semantics) |
| `role="alertdialog" aria-modal="true"` on an inline strip | **Accepted with rationale**: visually inline, but the backdrop blocks pointer input and Tab is trapped, so it is interaction-modal; a comment in the JSX records this. `research.md` rejected a visually heavy modal, not modal *semantics* |
| Font tiers 11/10/9 and non-monospaced count | **Accepted**: all within the four-tier system; the count is a button label, not a data readout |

## Spec review

| # | Finding | Disposition |
|---|---|---|
| P1 | R-03 runbook/evidence absent | **Fixed**: `acceptance.md` + `evidence/` |
| P2 | About idle hint missing 「不自动下载」 | **Fixed**; UIA found the exact string; `08-about-copy.png` |
| P3 | `取消` is not literally at the eraser's old coordinates | **Accepted deviation, safety property preserved**: `取消` is the far-right action of the inline strip; the eraser's old coordinates are covered by the cancel-on-click backdrop. A physical second click there cancels and deletes nothing (`03b-double-click-safe.png`, DB 4/1). The spec's *outcome* is met; the literal layout differs. |
| P4 | Workflow-docs commit counted as scope creep | **Not pilot scope**: `9a438f3` was approved separately; product review fixed point is `9a438f3` |
| P5 | `.status-button:focus-visible` broader than confirmation | **Kept and scoped**: needed for focus return to the eraser after cancel; it is one rule on status buttons only, not a whole-app retrofit |
| P6 | `role="status"` region mounted with its content | **Fixed**: always-mounted region with `aria-live="polite"`, hidden with `:empty`; UIA found the message |
| P7 | Chinese/English sentence parity | **Fixed**: English now says “nothing is downloaded or installed” (no “automatically”) |

## Real-testing finding

The UIA acceptance run caught a bug that static review missed:

- **Symptom**: after arming, pressing `Tab` from `取消` moved focus to
  `区域截图` in the status bar instead of `清空 4 条`.
- **Cause**: the trap's `first`/`last` refs were assigned in the opposite order
  from the DOM (`confirm` comes first, `cancel` last), so neither wrap condition
  matched.
- **Fix** (`fb25df6`): build the focusable list in DOM order
  `[confirm, cancel]` and cycle with a modulo, preventing every `Tab` inside the
  strip.
- **Retest**: focus `取消` → `清空 4 条` → `取消`; all acceptance checks pass.

This finding is recorded because it is exactly the class of issue the real-app
acceptance phase exists to catch.

## UI / design review

Screenshots reviewed at 420×600:

| Evidence | Review |
|---|---|
| `02-confirm-armed.png` | Inline strip above the status bar; not a full-screen modal. Destructive `清空 4 条` sits left, safe `取消` far right with a visible 2px focus ring. Copy is 11/10/9px, uses existing tokens; danger styling uses error color, not a new palette. |
| `03-cancelled.png` | Focus returns to the eraser and the focus ring is visible; panel content unchanged. |
| `03b-double-click-safe.png` | A click at the eraser's old coordinates leaves the panel unarmed; no data change. |
| `05-after-clear-toast.png` | `已清空 4 条记录` status pill visible; empty state; status bar 0/300. |
| `06-empty-after-clear.png` | Eraser disabled after clear; empty-state copy unchanged. |
| `07-confirm-opacity-50.png` | At 50% panel opacity the confirmation surface and text remain legible; the strip still reads as a foreground layer. |
| `08-about-copy.png` | About idle hint and description show the scoped GitHub statement; no absolute offline claim. |

Minor UI residual: the success toast overlaps the filter row for 2.4s. It is
legible and transient; noted for polish, not blocking.

## Requirement completion

The per-criterion matrix, commands, environment, and verdicts are in
`acceptance.md`. Summary:

| Requirement | Verdict |
|---|---|
| R-01 clear-all confirmation | **Verified** for arm/cancel/confirm/keyboard/focus/double-click/reset/success; **partial** for failure injection, screen-reader manual, and 150% DPI |
| R-02 scoped network claim | **Verified** |
| R-03 Rust regression + runbook | **Verified** |

## Verdict

No blocking review findings remain. The remaining items are explicitly listed as
residual risks in `acceptance.md`; the maintainer signed off on 2026-10-22 with
those residuals. Release remains a separate decision. No release, tag, push, PR,
or GitHub issue write was performed.
