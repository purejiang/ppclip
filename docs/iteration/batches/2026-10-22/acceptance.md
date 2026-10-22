# Acceptance — 2026-10-22 (local dry-run)

- **Status**: real-app acceptance complete; signed off by the maintainer on
  2026-10-22 with non-blocking notes. Screen-reader verification was explicitly
  downgraded by the maintainer; release is pending an explicit go-ahead.
- **Batch**: `2026-10-22`.
- **Requirements**: `requirements.md` (R-01, R-02, R-03).
- **Implemented on**: `iteration/v0.3.0`, product commits `fb25df6` and
  `3ce7531` (the latter keeps the empty live region in the accessibility tree).
- **Verdict**: R-01 **verified**, including the real failure path and a simulated
  1.5× render; R-02 **verified**; R-03 **verified**. Screen-reader verification is
  not part of this release gate (maintainer decision); the `role="status"`
  markup is retained.
- **Release**: not released, tagged, pushed, or opened as a PR. Sign-off does
  not authorize those actions. No GitHub issue was created, commented,
  labelled, or closed.

## 中文摘要

在隔离数据目录里用真实 release 包跑完了清空确认的主路径：第一次点橡皮擦
不删数据；确认条出现；取消 / Esc / Enter（焦点在取消上）/ 在橡皮擦旧位置
再点一下都不会删；Tab 只在「取消 / 清空」之间循环；确认后 4 条记录和
1 个贴图都被删、贴图窗口关闭、状态提示出现、空列表下橡皮擦禁用；隐藏面板
再打开会重置确认态；50% 面板不透明度下确认条仍可读；关于页两处新文案
都能被 UIA 读到。失败路径（数据库写失败）没有做故障注入，读屏软件和
150% DPI 用 1.5× 渲染模拟验证；读屏播报按维护者决定降级，不做人工验收。
失败路径已补测：外部锁住数据库时清空失败，确认条保持打开、错误显示、按钮
恢复可点、数据仍是 4 条 / 1 个贴图。维护者已签收；本轮不发布，发布决定
另行确认。

## Environment

| Item | Value |
|---|---|
| OS | Windows, interactive desktop session |
| Screen | 1920×1080, panel reported `420×600` (100% scaling) |
| Build | Tauri 2 · React 19 · Vite · Rust release build |
| App version | `0.2.5` tested; version-only bump to `0.2.6` for release |
| Isolated data | `docs/iteration/.runtime/acceptance-run/` (gitignored) |
| Seed | 4 text clips + 1 pin, inserted with `sqlite3` after schema creation |
| UI driver | `System.Windows.Automation` (UIAutomationClient) + `SendKeys` + physical click |
| Evidence | `evidence/` in this batch |

> **Environment note.** In this sandboxed session, starting the release build
> with `PPCLIP_DATA_DIR` under `%TEMP%` or `%LOCALAPPDATA%` failed at WebView2
> creation (`0x8000FFFF`); a repo-local writable directory worked. The normal
> `%APPDATA%` path also starts. This is recorded as a follow-up environment
> observation, not as a regression from R-01.

## Build and seed

```powershell
npm run lint          # 0 warnings, 0 errors
npm run build         # tsc -b && vite build
cd src-tauri
cargo test            # 68 passed, 0 failed
cargo check           # ok
cargo build --release # target/release/ppclip.exe
```

Seed used for the acceptance run:

```sql
INSERT INTO clips (kind,hash,text,preview,source,created_at,last_used_at,use_count) VALUES
('text','h1','Acceptance clip one','Acceptance clip one','clipboard','2026-10-22T10:00:00+08:00','2026-10-22T10:00:00+08:00',1),
('text','h2','Acceptance clip two','Acceptance clip two','clipboard','2026-10-22T10:00:00+08:00','2026-10-22T10:00:00+08:00',1),
('text','h3','Acceptance clip three','Acceptance clip three','clipboard','2026-10-22T10:00:00+08:00','2026-10-22T10:00:00+08:00',1),
('text','h4','Acceptance clip four','Acceptance clip four','clipboard','2026-10-22T10:00:00+08:00','2026-10-22T10:00:00+08:00',1);
INSERT INTO pins (clip_id,x,y,w,h,zoom,created_at)
VALUES (1,120,120,320,200,1.0,'2026-10-22T10:00:00+08:00');
```

The panel was shown by launching a second instance; the first instance owns the
tray/panel. The panel was pinned for the automated run so focus changes could
not hide it; the confirmation behaviour is independent of pinning.

## Checks and evidence

| # | Check | Result | Evidence |
|---|---|---|---|
| 1 | Seed restored: 4 clips, 1 pin, pin window visible | PASS | `01-panel-list.png` |
| 2 | First click arms; DB still 4 clips / 1 pin; no deletion | PASS | `02-confirm-armed.png` |
| 3 | Strip shows `4`, `1 个桌面贴图`, image/thumbnail deletion, irreversible, source files untouched | PASS | `02-confirm-armed.png` |
| 4 | Initial focus is `取消`; focus ring visible | PASS | `02-confirm-armed.png`, UIA log |
| 5 | `Tab` → destructive button; `Shift+Tab` → `取消`; focus does not escape | PASS | UIA log (focus `取消` → `清空 4 条` → `取消`) |
| 6 | `取消` closes, DB unchanged; focus returns to eraser | PASS | `03-cancelled.png` (focus ring on eraser) |
| 7 | Second click at the eraser's old coordinates cancels, does not confirm | PASS | `03b-double-click-safe.png`; DB 4/1 |
| 8 | `Enter` while focus is on `取消` cancels, does not delete | PASS | UIA log; DB 4/1 |
| 9 | `Esc` cancels, panel stays visible, DB unchanged | PASS | UIA log; DB 4/1 |
| 10 | Hide while armed, reopen: no confirmation, data unchanged | PASS | `04-reopened-unarmed.png`; DB 4/1 |
| 11 | Confirm: DB 0 clips / 0 pins; pin window and viewer window closed | PASS | `05-after-clear-toast.png`; UIA log |
| 12 | Success status message `已清空 4 条记录` and empty state | PASS | `05-after-clear-toast.png`, `06-empty-after-clear.png` |
| 13 | Eraser disabled when history is empty | PASS | UIA `没有可清空的历史`, `IsEnabled=false` |
| 14 | Confirmation legible at 50% panel opacity | PASS | `07-confirm-opacity-50.png` |
| 15 | About idle hint and description contain the scoped network statement | PASS | `08-about-copy.png`; UIA found both strings |
| 16 | Full UIA assertion sequence | PASS | `acceptance-log.txt` |
| 17 | Clear failure (external DB write lock): strip stays, error shown, buttons re-enabled, DB unchanged | PASS | `09-clear-failure.png`, `09-clear-failure-log.txt` |
| 18 | Simulated 150% render (WebView2 scale 1.5 + 630×900 physical) | PASS | `11-confirm-150dpi-simulated.png` |

### Log summary

The UIA run finished with `ALL ACCEPTANCE CHECKS PASSED`. Key database
assertions:

- after arm: clips 4, pins 1;
- after cancel/Enter/Esc/double-click: clips 4, pins 1;
- after confirm: clips 0, pins 0;
- pin window present after startup, absent after confirm;
- viewer opened on a row (`750,216,760,620`) and was absent after confirm;
- failure injection: an external `BEGIN EXCLUSIVE` lock made the clear return
  `清空失败：数据库错误: database is locked`; clips stayed 4, pins stayed 1, the
  strip stayed open, and the confirm button was enabled again;
- simulated 1.5× render: the panel was resized to `630×900` physical with
  WebView2 device scale 1.5; the 420×600 logical layout and confirmation strip
  rendered correctly.

The full UIA assertion log is `evidence/acceptance-log.txt` (the per-button debug
dump was removed). The automation script itself lives in the gitignored
`docs/iteration/.runtime/` and is not committed.

## Requirement completion

### R-01 — clear-all confirmation

| Acceptance criterion | Verdict | Notes |
|---|---|---|
| One click opens confirmation, no deletion | verified | checks 2, 3 |
| Copy: count, pins close, image/thumb files, source files, irreversible | verified | copy rendered; pin count; Rust test covers file removal; file-source untouched is code inspection |
| Cancel far right; double-click safe | verified with deviation | cancel is the far-right action of the inline strip; the eraser's coordinates are covered by the backdrop, which cancels. This preserves the double-click safety property; check 7 |
| Initial focus cancel; Enter only focused button | verified | checks 4, 8 |
| Esc order and focus return | verified | checks 6, 9 |
| Tab cycles two buttons | verified | check 5; a real UIA run caught a wrong trap condition, fixed in `fb25df6` |
| Rest of panel inert | verified | backdrop + focus trap; physical click at old eraser position cancels |
| Eraser disabled when stats unknown/0 | verified for 0; code inspection for unknown | check 13; initial loading label/disabled path reviewed |
| Success refresh/status/focus | verified | checks 11, 12; UIA found `已清空 4 条记录` |
| Failure keeps strip, shows error, re-enables | verified | check 17; external DB write lock, DB unchanged |
| Reset on hide | verified | check 10 |
| 50% opacity legible | verified | check 14 |
| Reduced motion | code inspection only | CSS `prefers-reduced-motion` guard |
| 150% DPI | verified by simulation | check 18; 1.5× render at 630×900; real OS DPI switch not performed |
| Screen reader announcement | downgraded / not required | maintainer explicitly excluded this check; `role="status"` markup retained, no human AT pass |

### R-02 — scoped network claim

| Acceptance criterion | Verdict | Notes |
|---|---|---|
| README.md and README.en.md no absolute "no network" claim | verified | grep: both now say offline by default, one manual GitHub check, no clipboard upload/download/install |
| About idle hint and description updated | verified | `08-about-copy.png`; UIA exact strings |
| Known-limitation line in both READMEs | verified | both languages |
| Feature unchanged (check still only checks) | verified | no Rust/network code changed |

### R-03 — regression test and runbook

| Acceptance criterion | Verdict | Notes |
|---|---|---|
| Rust unit test clears pins and image/thumbnail files | verified | `clear_removes_pins_and_image_files`; 68/68 tests pass |
| Runbook with isolated `PPCLIP_DATA_DIR` | verified | this file |
| Every R-01 criterion has an evidence row/verdict | verified | table above; remaining notes are non-blocking |

## Residual risks and follow-ups

1. **Real OS 150% DPI switch not performed.** The 1.5× render was simulated with
   WebView2 device scale plus a 630×900 physical window; layout and copy were
   correct. A real display-scaling switch is a low-risk manual check.
2. **Screen reader intentionally excluded.** The maintainer decided not to gate
   this release on a screen-reader pass. The `role="status" aria-live="polite"`
   markup is retained; no NVDA/Narrator narration was performed.
3. **Toast placement.** The 2.4s success toast sits over the filter row; it is
   legible but slightly covers the filters. Acceptable for this pilot, noted for
   UI polish.
4. **Custom data dir under `%TEMP%`/`%LOCALAPPDATA%`.** WebView2 failed in this
   sandboxed session; repo-local path worked. Normal app data dir works. Treat as
   a separate environment/follow-up, not part of R-01.

## Sign-off

- Maintainer: purejiang
- Date: 2026-10-22
- Decision: **accept for release**, with non-blocking notes: screen-reader
  verification is explicitly excluded by the maintainer; real OS 150% DPI
  switching was simulated, not switched; toast/custom-dir observations remain.
- Scope: implementation and this acceptance record. Sign-off does **not**
  authorize push, tag, release, or any GitHub write; those require a separate
  explicit go-ahead.
