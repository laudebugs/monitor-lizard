# T1b: Page shell, sidebar groups and table styles for every page

| Field | Value |
|---|---|
| ID | `T1b` |
| Status | `ongoing` |
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

- [ ] Add the title-sentence functions with tests first.
- [ ] Convert each page renderer to call `pageShell`; keep its body unchanged.
- [ ] Check every page at 1360 px and 400 px in both themes.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Page with no inspector | Body takes the full width |
| Count above 99 | Badge shows `99+` |
| Title longer than the width | Wraps; never clips |
| 400 px width | Inspector stacks below the table |
| Keyboard | Skip link to the main content; focus order breadcrumb, search, filters, table |

## Acceptance criteria

- All ten pages share the shell and look consistent.
- Every badge links to the pre-filtered page it counts.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
