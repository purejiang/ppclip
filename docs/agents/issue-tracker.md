# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues in `purejiang/ppclip`.
The canonical interface is the `gh` CLI; a read-only REST fallback is documented
at the bottom for restricted sandboxes.

## Conventions

- Infer the repo from `git remote -v`; it should resolve to `purejiang/ppclip`.
- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a
  heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --json number,title,body,labels,comments`.
- **List issues**:
  `gh issue list --state open --json number,title,body,labels,comments`.
- **Comment**: `gh issue comment <number> --body "..."`.
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` /
  `--remove-label "..."`.
- **Close**: `gh issue close <number> --comment "..."`.
- **Sub-issues / blocking**: use GitHub's native sub-issue and dependency
  relationships when available; otherwise put `Part of #<parent>` and
  `Blocked by: #<n>` at the top of the child body.
- **PRs as a request surface**: **no.** External pull requests are not triaged
  as feature requests; use issues for requirements.
- **Wayfinding**: not used in this repo. A batch of issues is the unit of work,
  not a wayfinder map.

The five triage roles and their label strings are defined in
[`triage-labels.md`](./triage-labels.md). Labels not present in the repository
must be created once, not silently assumed.

## Read-only fallback (restricted sandboxes)

If `gh` cannot run (sandbox, no auth, or output capture blocked):

- Read with the GitHub REST API:
  `https://api.github.com/repos/purejiang/ppclip/issues?state=open&per_page=100&sort=updated&direction=desc`.
  Save the raw JSON outside tracked docs (for example `docs/iteration/.runtime/`).
- For issue comments:
  `https://api.github.com/repos/purejiang/ppclip/issues/<n>/comments?per_page=100`.
- Writes (create/comment/label/close) are **not** part of the read-only fallback.
  Produce a paste-ready title/body/label set, or run `gh` from a normal terminal.
- Never put tokens, real clipboard contents, or private data into URLs, docs, or
  commits. Never invent issue numbers.

## When a skill says "publish to the issue tracker"

Create a GitHub issue. If the environment cannot create issues, write the same
content to the batch directory as a paste-ready block and mark it unpublished;
do not pretend it exists on GitHub.

## When a skill says "fetch the relevant ticket"

Read it as in **Read an issue** above.
