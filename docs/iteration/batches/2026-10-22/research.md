# Research — 2026-10-22 (local dry-run)

Answers the four questions raised by `intake.md` / `triage.md` against primary
sources, and turns them into adopt / adapt / reject calls for L-01, L-02, L-03.
Sources fetched with `web_fetch` on **2026-10-22**; `web_search` was not used.
Failed URLs are recorded under *Evidence gaps* rather than papered over.

## Sources

| # | Title | URL | Accessed | Takeaway |
|---|---|---|---|---|
| S1 | NN/g — Confirmation Dialogs Can Prevent User Errors — If Not Overused | https://www.nngroup.com/articles/confirmation-dialog/ | 2026-10-22 | Confirm only serious / hard-to-undo actions; be specific about the consequence; label buttons with the outcome, not Yes/No; give no default "Yes"; never confirm routine actions. |
| S2 | W3C APG — Dialog (Modal) Pattern | https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/ | 2026-10-22 | Focus moves into the dialog on open, Tab stays inside, Esc closes, focus returns to the invoking control; for an irreversible final step put initial focus on the *least destructive* action; label via `aria-labelledby`/`aria-label`; `aria-modal` only if it really is modal. |
| S3 | Microsoft Learn — Dialog controls | https://learn.microsoft.com/en-us/windows/apps/design/controls/dialogs-and-flyouts/dialogs | 2026-10-22 | Every dialog needs a safe non-destructive button; button text names the specific response; the first line states the issue; Esc maps to the safe button. |
| S4 | WCAG 2.2 SC 2.2.1 Timing Adjustable (Understanding) | https://www.w3.org/WAI/WCAG22/Understanding/timing-adjustable.html | 2026-10-22 | Content-set time limits need turn-off / 10x adjust / 20 s extend; "any process that happens without user initiation after a set time" counts as a time limit. |
| S5 | WCAG 2.2 SC 2.4.13 Focus Appearance (AAA, with 2.4.7 Focus Visible) | https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html | 2026-10-22 | Indicator area ≥ a 2 px perimeter of the control, and ≥ 3:1 contrast change between focused and unfocused states. |
| S6 | WCAG 2.2 SC 1.4.11 Non-text Contrast (AA) | https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html | 2026-10-22 | 3:1 against adjacent colours for whatever identifies a control and its state, focus indicator included. |
| S7 | WCAG 2.2 SC 4.1.3 Status Messages (AA) | https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html | 2026-10-22 | Non-focus text about results, counts or errors must be programmatically determinable (`role="status"` / live region); a dialog is a change of context, not a status message. |
| S8 | WCAG 2.2 SC 2.3.3 Animation from Interactions (AAA) | https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html | 2026-10-22 | Interaction-triggered non-essential motion must be disable-able; sufficient technique C39 is `prefers-reduced-motion`. |
| S9 | NN/g — Skeleton Screens 101 | https://www.nngroup.com/articles/skeleton-screens/ | 2026-10-22 | Skeleton for full-page loads under ~10 s; nothing at all when the load is under 1 s (the flash is worse); spinner for a single module; a mostly-blank "frame" skeleton reads as broken. |
| S10 | RFC 2119 / BCP 14, §6 Guidance in the use of these Imperatives | https://www.rfc-editor.org/rfc/rfc2119.html | 2026-10-22 | Absolute imperatives are for absolute requirements and "must be used with care and sparingly" — an unscoped absolute product claim has MUST-NOT strength and needs an exception clause when behaviour is conditional. |

## adopt / adapt / reject

1. **Adopt — confirmation, not undo, for clear-all in v1.** S1 #1: confirm actions that cannot be undone; undo is the better safety net *where it exists*, and a soft-delete model is explicitly out of scope. The gap is stated, not hidden.
2. **Adopt — state the consequence, not "are you sure".** S1 #3/#4, S3: name the real effects with real counts — clips removed, pinned windows closed, image files deleted from disk (`src-tauri/src/storage.rs:309-337` does delete them) — and say it cannot be undone, because that is literally true today. Button labels are outcomes: `清空 N 条` / `取消`, never OK/Cancel.
3. **Adopt — no default on the destructive control.** S1 #6 + S2's "least destructive action" note: initial focus goes to the safe control, so Enter cannot wipe history by reflex.
4. **Adopt — Esc cancels at its own layer.** Today Esc is about → settings → hide panel (`src/windows/PanelApp.tsx:204-209`); the confirmation state must sit above the list and Esc must cancel the confirmation *without* also hiding the panel.
5. **Adopt — focus contract.** S2: focus enters the confirmation, is contained while it is open, and returns to the eraser button on close (unless that control no longer exists).
6. **Adopt — a real focus indicator (new work).** S5/S6: `src/styles/index.css` currently has exactly one `:focus-visible` rule (`.settings-card--link`, line 2520) and no global one. The confirmation must not be the one control you cannot see focused; target 2 px + 3:1 change.
7. **Adapt — reuse the ClipRow two-step, drop its 3 s timer.** The row pattern (`src/components/ClipRow.tsx:217-232`, `PanelApp.tsx:84-98`) is the house style and should stay familiar. But S4: a state that auto-reverts is a time limit; 3 s is not enough to read a consequence sentence. Adapt to no timer, or ≥ 20 s with a visible warning.
8. **Adapt — announce the outcome.** S7: after a successful clear, a `role="status"` message ("已清空 N 条") gives non-visual users the same result. The panel has no live region today (only the viewer toast, `ViewerApp.tsx:314`).
9. **Adapt — respect reduced motion.** S8: `index.css` has zero `prefers-reduced-motion` rules while `about-fade-in`, `toast-in` and the infinite `status-pulse` animate unconditionally. Ship any new confirmation transition behind the media query; retrofitting the existing three is cheap.
10. **Adapt — "unknown" is a third state, not "empty".** S9: skeleton for the full-list surface, nothing when the load is under 1 s (local SQLite usually is), spinner only for a module. Concrete rule: a `loaded` flag gates the placeholder; a short delay avoids flashing the skeleton.
11. **Reject — nonstandard confirmation (type "DELETE").** S1 #7 reserves it for the most dangerous and rare actions; here it would break the panel's inline two-step vocabulary and force English typing. Revisit only if clear-all becomes worse than irreversible.
12. **Reject — a modal dialog by default.** S1 #2: confirmations at modal weight train dismissal. The panel is 420×600 and already carries one overlay (AboutPage); an inline status-bar state stays inside the existing Esc chain and focus order. *If* a modal is chosen instead, S2's contract becomes mandatory, not optional.

## Implications

### L-01 — clear-all confirmation
- Draft copy (Chinese, maintainer to review): title 「清空全部历史？」; body 「将删除 N 条历史记录，其中 M 条已贴在桌面、会一起关闭；磁盘上的图片文件也会删除。此操作无法撤销。」; buttons 「清空 N 条」/「取消」.
- `N`/`M` come from `stats`; while `stats` is `null` the eraser should be disabled rather than printing `0` (`PanelApp.tsx:435`).
- First click only opens the state; nothing is deleted until the explicit confirm (`PanelApp.tsx:285-295` currently calls `api.clearClips("all")` on the first click).
- Worth adding to the spec's acceptance criteria: Enter does not confirm by default; Esc cancels at the confirmation layer; focus returns to the eraser; success is announced via `role="status"`; the whole flow is keyboard-only.
- Leave `pendingDelete` (row delete) alone this batch — two mechanisms with different weight is the correct outcome here, not an inconsistency to iron out.

### L-02 — README network wording
- Current claims: `README.md:34` 「数据全部保存在本机，应用不联网」; `README.md:112` 「应用也不发起网络请求」; `README.en.md:32` "PPClip makes no network requests."
- Accurate behaviour: exactly one request path, `check_latest_release` → WinHTTP GET `api.github.com/repos/purejiang/ppclip/releases/latest`, only when the user presses 检查更新 (`src-tauri/src/commands.rs:89`, `src-tauri/src/github.rs:5-20`, `src/components/AboutPage.tsx:52-68`). `open_url` hands the URL to the browser, so the app itself sends nothing. `github.rs` already words this truthfully — the README is the stale artifact.
- S10 framing: the absolute claim is a MUST-NOT-strength sentence with no exception clause, while the code has exactly one exception with a trigger. Fix the scope, do not weaken the feature.
- Suggested wording (zh): 「数据全部保存在本机，不上传、不同步。应用默认不联网；仅当你点击「关于 → 检查更新」时，会请求 GitHub Releases 接口（`api.github.com`），只检查版本，不下载、不安装。」 (en): "Everything stays on your machine, and PPClip is offline by default. It contacts GitHub's releases API only when you press *Check for updates* — nothing is downloaded or installed automatically."
- Add one line to 已知限制: 检查更新需要联网，失败不影响任何本地功能.

### L-03 — false empty state
- Code supports the suspicion (`clips` starts `[]`, placeholder at `PanelApp.tsx:405-411`, async `refresh` at `:114-135`), but the common path hides it: `main` is created hidden, the webview mounts at startup and `refresh()` runs at mount, so the first list usually lands long before the user opens the panel.
- The reachable path is **startup with `panel_pinned` on**: `src-tauri/src/lib.rs:1130-1131` applies pinned and calls `show_panel` during setup, so the panel is on screen while the first `refresh()` is still in flight. That is the scenario a runtime test must target.
- Two false-empty sub-states exist independently of timing: `stats?.total ?? 0` prints "0 / 300 条上限" before stats arrive (`PanelApp.tsx:435`), and a failed `refresh()` leaves `clips` empty, so the placeholder reads "还没有记录" beside the error banner (`:132-134`).
- Design fix: make loading its own state (`loaded === false` → skeleton rows or nothing, never "还没有记录…"); keep "没有匹配的记录" for a query that genuinely returned zero.
- Keep L-03 at `needs-info`: this narrows *what* to reproduce; it does not replace the reproduction.

## Evidence gaps / runtime work still needed

- **Failed URLs (unresolved):** `https://www.nngroup.com/articles/undo/` → HTTP 404 (no NN/g article at that path — do not cite it); `https://developers.google.com/style/timeless-documentation` → fetch failed. The only primary statement on undo used here is S1's "do try your best to offer undo", so a dedicated undo-vs-confirm source is still missing.
- **No docs-style authority on network wording** was retrievable, so L-02's wording rests on S10 plus the code's own documented behaviour — reasoned, not authoritative.
- **L-03 needs a real reproduction:** launch with `panel_pinned` on (or drive `--smoke-panel`), capture the panel in roughly the first 200 ms, and record whether "还没有记录" is visible. Until then the batch must not call it confirmed.
- **L-01 needs runtime checks code alone cannot settle:** keyboard-only round trip, focus return to the eraser, Esc layer order, and whether the confirmation fits 420×600 without pushing the status bar out of view.
- **Contrast is unmeasured:** the numbers above are spec thresholds; `--error #ff6b6b`, the danger button and any new focus ring still need an actual contrast calculation against the panel background before the spec calls them compliant.
- **Screen-reader behaviour** (NVDA / Narrator) for the confirmation and the new live region is unverified in this environment.
