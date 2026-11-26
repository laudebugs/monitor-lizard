# T1b: Page shell, sidebar groups and table styles for every page

| Field | Value |
|---|---|
| ID | `T1b` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11 |
| Depends on | T1a, N1 |
| ADRs | None |
| Branch | `feat/t1b-page-shell-sidebar-tables` |
| PR title | `feat(web): one page shell, grouped sidebar and shared table styles` |
| Size | M (200-400) |
| PR | — |

## Goal

Give every page the same anatomy: breadcrumb and search on top, a verdict line as the title, filters, a table and an inspector. Group the sidebar and show counts of items that need a decision. This todo implements plan decision D11.

## Context

- Design boards F, G and H show the shell; N1 provides the hash state and the breadcrumb component.
- Apps, Listeners, Privilege, History and Performance keep their own content and get only the shell, so the app looks consistent before they are redesigned.

## Scope

**In scope:**

- A `pageShell({ crumb, title, subtitle, actions, filters, body, inspector })` function that every page's renderer calls.
- Sidebar groups (Security, Machine, Help) and count badges for Overview, Apps and Persistence, computed from data already loaded.
- Shared table and inspector classes: sticky header, row selection state, mono tabular numbers, relative time with the absolute time on hover.
- Each page supplies a title sentence; where a page has no verdict yet it uses its name.

**Out of scope** (name the todo that covers it):

- New content on Apps, Listeners, Privilege, History: not planned until they have designs.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `pageShell`, `NAV` groups, table and inspector CSS |

## Steps

- [x] Add the title-sentence functions with tests first.
- [x] Convert each page renderer to call `pageShell`; keep its body unchanged.
- [x] Check every page in both themes at the tab's width (1019 px) and at 400 px. 1360 px was not available (see the Implementation Record).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Page with no inspector | Body takes the full width |
| Count above 99 | Badge shows `99+` |
| Title longer than the width | Wraps; never clips |
| 400 px width | Inspector stacks below the table |
| Keyboard | Skip link to the main content; focus order breadcrumb, search, filters, table |

## Acceptance criteria

- All nine pages share the shell and look consistent. Checked at 1019 px and 400 px; not at 1360 px (the window cannot be resized).
- Every badge links to the pre-filtered page it counts. Overview has no badge, because it has no filtered page to link to.

## Tests

Logic tests for titles and counts, 6 minimum. A test that every `NAV` entry has a group. Visual checks by hand and recorded.

## Verification

```sh
make check
make test
cargo run --release -- serve   # visit every page in both themes
```

## Docs to update

`docs/agents/layout.md` if it names the page structure; plan status.

## Risks and open questions

- Every table carries a three-line export note, which looks heavy (seen at 1352 px in the T1a sweep). Consider a one-line note or a tooltip here and in the redesigns that follow (R1, R2, R3b). A design note, not a bug.
- Converting ten renderers is mechanical but touches most of the file; keep the diff to the call sites.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26. Built in `web/index.html` with tests in `tests/web/page-shell.test.mjs`.

**Built.**

- `pageShell` draws the title, subtitle, filters, body and inspector. All nine renderers call it.
- The sidebar has Security, Machine and Help groups. Apps, Privilege and Persistence show count badges that link to their filtered pages. A count above 99 reads `99+`.
- Titles are verdict sentences from pure functions, such as "9 apps need review" and "Nothing needs review". History, Performance and Guide use their names.
- Times in Persistence, Network, History and sudo launches read "5m ago" with the exact time in the title.
- The export note is one line. A "File details" button opens the full text, which stays in the page.
- A skip link and shared inspector classes were added.

**Deviations.**

- The Privilege badge counts all failed authorization requests, not only sudo ones, so it equals the rows its link shows. The Overview tile still counts sudo.
- The top bar holds breadcrumb and search and is not redrawn, so typing keeps focus. The shell returns the breadcrumb separately.
- The skip link is a button, since a `#` link would change the page state.
- The sidebar is rebuilt only when it changes, and keeps focus when it is.
- Overview has no badge.
- Table row grids and widths are untouched.

**Checked in Chrome** (this build, scratch HOME, port 4972, tab width 1019 px, window not resized):

- Dark: Overview, Apps (filtered), Network, Listeners, Persistence, History, Performance, Guide. Light: Overview, Apps with an app selected, Network, Listeners, Privilege, Persistence, History, Performance, Guide.
- Table check on every page: rows, cells and Copy buttons line up, and zero console warnings.
- The Apps badge opened Apps with the Needs review chip on. The disclosure opened and closed, kept focus and set `aria-expanded`.
- Focus order was skip link, sidebar, breadcrumb, search, filter chips, then table. Enter on the skip link moved focus to the main area, and Tab then reached the breadcrumb. The address stayed the same.
- A 400 px same-origin frame, all nine pages in both themes: scroll width 400 each time. The Apps inspector stacked below the table. The Listeners title wrapped and the disclosure fit.

**Not checked.**

- 1360 px width, Privilege and Apps in dark at the tab width, the inspector beside the table, forced colors, and a screen reader.
- The Network table's first columns still collapse; that bug has its own fix.
- Keyboard focus in the table is lost on each poll redraw. It was not changed.
- Authorization times stay as time of day, because the log gives text, not seconds.
- `/code-review` and `/writing-review` are for the owner's session.
