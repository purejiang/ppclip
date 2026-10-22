# PPClip Glossary

The project's domain vocabulary. Use these terms in issues, specs, task titles,
test names, review comments, and code comments. If a concept is missing, do not
invent a synonym; note the gap for `domain-modeling`.

## Product concepts

| Term | Definition |
|---|---|
| **Clip** | One clipboard-history entry. Kinds: `text`, `image`, `file`. Deduplicated by content hash; a repeat copy bumps `last_used_at` / `use_count` instead of inserting. |
| **File clip** | A group of files copied from Explorer (`CF_HDROP`). Paths are stored as a JSON array in `clips.text`; `Clip.files` is the parsed view. |
| **Source** | Where a clip came from: `clipboard` or `screenshot`. Shown as a badge in the panel. |
| **Pin** | A desktop floating window that displays a clip (`pin-<id>`). Has its own position and zoom, restored on startup. Only text and image clips can be pinned. |
| **Ball / floating entry** | The `ball` window: the always-on-top desktop entry with three forms. Not a taskbar window. |
| **Tab / capsule** | The docked form of the ball: a narrow shape attached to a screen edge. |
| **Orb** | The regular, undocked form of the ball. |
| **Wheel** | The HUD radial menu expanded from the orb after hover; contains screenshot and panel actions. |
| **Anchor** | The physical-pixel center of the current shape. The single source of truth for ball geometry; window position is derived from it. |
| **Dock** | Snap the ball to a screen edge and switch it to the tab form. Persisted as `ball_docked`. |
| **Panel** | The main history window (`main`): search, filters, clip list, status bar, settings screen, About overlay. |
| **Viewer** | The single content window (`viewer`) that shows the full text, image, or file details of a clip. |
| **Overlay** | The region-capture window: a frozen screenshot plus four dimming masks and a selection rectangle. |
| **Self-write** | The app's own clipboard write. The listener suppresses the matching hash/time window so the app does not record its own copy action. |
| **Eviction** | Cleanup when history or image-storage limits are exceeded. Eviction skips clips referenced by pins. |
| **Thumbnail** | The small preview stored in `thumbs/<hash>.png` for image rows. The original stays in `images/<hash>.png`. |
| **Self-write suppression** | The mechanism behind **self-write**: hash match first, then a short time window for re-encoded images. |

## UI and design terms

| Term | Definition |
|---|---|
| **Glacier** | The hand-written design system in `src/styles/index.css`: dark glass cards, ice-blue primary, aurora tokens for the ball. No UI library. |
| **Glass card** | The window shell: rounded 2xl surface at 85% opacity, blurred backdrop, outline-variant border. |
| **Glow** | The two ambient blobs in each window's main area (primary top-right, tertiary bottom-left). |
| **Pill filter** | The All / Text / Images / Files chips above the clip list. |
| **Status bar** | The panel strip showing `n / limit`, screenshot buttons, and clear-all. |
| **About page** | The in-panel overlay with version, update check, repository link, and license. |
| **Settings screen** | The second screen of the panel window, not a separate window. |

## Workflow terms

| Term | Definition |
|---|---|
| **Batch** | One issue-intake cycle: a snapshot of open issues plus the artifacts produced from them. Identified by date, e.g. `2026-10-22`. |
| **Intake** | Trigger check, issue snapshot, normalization, and new/untriaged classification. Owned by `docs/iteration/INTAKE.md`. |
| **Council** | The multi-role analysis step: product, UX, engineering, QA, security; independent takes, then a chair synthesis and a critic. Owned by `docs/iteration/COUNCIL.md`. |
| **Requirement** | An approved, testable statement of what the batch will change, linked to source issues. Produced with the `to-spec` template. |
| **Task graph** | The ordered set of vertical slices with explicit `blocked_by` edges. Produced with the `to-tickets` rules. |
| **Acceptance** | Real-app validation of each requirement against its acceptance criteria, with evidence and a `verified` / `partial` / `failed` verdict. Owned by `docs/iteration/ACCEPTANCE.md`. |
| **Bootstrap** | The zero-issue path: audit the current product, propose candidate issues, get human approval, create or keep them locally, then enter the normal loop. |
| **Delta** | A PPClip-specific addition that the generic skills do not cover: intake, council, acceptance, release, adapter values. |
