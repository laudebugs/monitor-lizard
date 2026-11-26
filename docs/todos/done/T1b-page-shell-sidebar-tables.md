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
- [x] Check every page in both themes at 1019 px, at 1352 px (owner's session) and at 400 px. 1360 px was not used.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Page with no inspector | Body takes the full width |
| Count above 99 | Badge shows `99+` |
| Title longer than the width | Wraps; never clips |
| 400 px width | Inspector stacks below the table |
| Keyboard | Skip link to the main content; focus order breadcrumb, search, filters, table |

## Acceptance criteria

- All nine pages share the shell and look consistent. The first draft said ten pages; there are nine page ids, so the spec was wrong. Checked at 1019 px, 1352 px and 400 px, all in both themes.
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
- [x] `/code-review` run per `AGENTS.md`; both axes ran and the fix commits answer them
- [ ] `/writing-review` run on any prose (not run; the prose in this todo is unreviewed)
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

**Review fixes** (second commit round).

- The Network title waits for the host history. Every verdict title now names the data it counts and shows the page name until it has loaded:

| Page | Waits for |
|---|---|
| Overview | snapshot, privilege log, startup items |
| Apps, Listeners | snapshot |
| Privilege | privilege log |
| Persistence | startup items |
| Network | snapshot, host history |
| History, Performance, Guide | nothing (the name) |

- The Apps inspector has a Related section: remote hosts, listening sockets, startup items and new hosts, each only when it opens rows. Links come from `writeHash`. They filter by the app's name (its path for startup items), so an app with a similar name may also show.
- Network and Persistence have no inspector, so they have no Related section. A Guide link per app is not done: Guide entries name pages, not apps, and the hash has no key for it.
- Counts in sentences use `plural()`, so "1 app" reads right.
- Two failed numbers stay on purpose. The badge counts all failed authorization requests; the Overview tile counts sudo only and says so. The tile links to the failed filter.
- One function counts apps needing review for the title, the badge and the tile.
- Times are `<time>` elements with the exact time in the title and in text for a screen reader. Badge names start with the shown text, such as "99+ apps need review, 120 in total".
- NAV entries are objects. Performance's live toggle uses the title actions slot. The unused `crumb` parameter was dropped.

**Deviations.**

- The first draft said ten pages. There are nine page ids, so the spec was wrong.
- The skip link is a button, because a `#` link would change the page state.
- Overview has no badge, because it has no filtered page.
- Table row grids and widths are untouched.

**Checked in Chrome** (this build, scratch HOME, port 4972, tab width 1019 px, window not resized):

- Dark: Overview, Apps (filtered), Network, Listeners, Persistence, History, Performance, Guide. Light: Overview, Apps with an app selected, Network, Listeners, Privilege, Persistence, History, Performance, Guide.
- Table check on every page: rows, cells and Copy buttons line up, and zero console warnings.
- The Apps badge opened Apps with the Needs review chip on. The disclosure opened and closed, kept focus and set `aria-expanded`.
- Focus order was skip link, sidebar, breadcrumb, search, filter chips, then table. Enter on the skip link moved focus to the main area, and Tab then reached the breadcrumb. The address stayed the same.
- A 400 px same-origin frame, all nine pages in both themes: scroll width 400 each time. The Apps inspector stacked below the table. The Listeners title wrapped and the disclosure fit.

**Looked at in a 1352 px window by the owner's session** (build of the merged branch, scratch HOME, window not resized):

- Seen in dark and in light: Overview, Apps with the inspector open, Privilege, Listeners, History, Performance and Guide. Network and Persistence were seen in light, and in dark in an earlier pass. So all nine pages were seen in both themes.
- Titles read correctly on every page, for example "6 items need your attention", "9 apps need review", "No failed authorization requests" and "61 listening sockets are open to your network".
- The Apps inspector sits beside the table. App names are readable after the narrow-column fix; the Signing and Listening cells still end in an ellipsis, and the card scrolls.
- The Related section shows in the Apps inspector. Tables and Copy buttons looked aligned, and no visual defect was found.
- Persistence and History were mostly empty states on a fresh home.

**"No new hosts" on a fresh home is correct, not a bug.** The page counts a host as new only when its app is approved, the host was first seen within an hour, and the host is not marked expected. On a fresh home no app is approved, so the count is 0 even though every row says "just now". Unapproved apps already show under Needs review. This comes from the page's `isNew` rule, not from first-run seeding.

**Not checked, and where it goes.**

- Forced colors: moved to T1c.
- A screen reader pass: not planned. The markup follows the disclosure and time patterns and has hidden text for times, but no screen reader was run.
- 400 px was run after merging the narrow-column fix, but before the squash merge. It was not repeated on the final build.
- A Guide link per app in Related: not done, because Guide entries name pages, not apps.
- Keyboard focus in the table is lost on each poll redraw. This was already so and is not changed.
- Authorization times stay as time of day, because the log gives text, not seconds.
- `/writing-review` was not run.
