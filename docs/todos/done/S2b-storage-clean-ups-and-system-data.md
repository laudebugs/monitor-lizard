# S2b: Storage page: suggested clean-ups and Inside System Data

| Field | Value |
|---|---|
| ID | `S2b` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D4, D9 |
| Depends on | S2a |
| ADRs | None |
| Branch | `feat/s2b-storage-clean-ups` |
| PR title | `feat(web): add suggested clean-ups and Inside System Data to Storage` |
| Size | M (200-400) |
| PR | [#54](https://github.com/laudebugs/monitor-lizard/pull/54) |

## Goal

Add the rest of the Storage page: a "Suggested clean-ups" list ranked by size, an "Inside System Data" breakdown, and an Export HTML report button.

## Context

- Design canvas "Sys Monitor Redesign Directions" (2026-11-26), board D.
- `GET /api/storage` (S1) already carries labels and sizes; this todo only renders them.

## Scope

**In scope:**

- Clean-ups: items labelled `safe` or `review` or `yours`, largest first, each with name, reason, label chip, size and a Review button that selects the item in the inspector. "Review all safe" selects every `safe` item.
- Inside System Data: caches, local snapshots, indexes, logs and other, each with a bar. A row whose size cannot be read shows the reason, never 0.
- Export HTML report: one self-contained file of the current view, built in the browser, with no remote assets.

**Out of scope** (name the todo that covers it):

- Moving anything to the Trash: S3b.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `storageCleanups()`, `systemData()`, `exportReport()` |

## Steps

- [ ] Write the ranking and the report builder as pure functions with tests first.
- [ ] Render the lists with the shared chips from T1a.
- [ ] Open the exported file offline and check it.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No `safe` items | "Nothing safe to remove" and the button is disabled |
| Snapshot size unavailable | Row shows the count and the reason |
| App name containing `<` or `&` | Escaped in the report |
| 10,000 items | List shows the top 50 with a "Show all" control |

## Acceptance criteria

- The sum of the clean-up sizes equals the sum of the selected items in the inspector.
- The exported report opens with no network and shows the same numbers.

## Tests

Logic tests, 8 minimum. Layout checked by hand in a real browser and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status.

## Risks and open questions

- The report lists file paths; the button says so before it downloads.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-26)

Built:

- Pure functions in the `storage-cleanups` block of `web/index.html`: `stCleanups`, `cleanupView` (top 50), `safeIds`, `pickedRows`, `sumSizes`/`sumText`, `systemData`, `reportHtml`. 19 tests in `tests/web/storage-cleanups.test.mjs`.
- Page: `storageCleanups()`, `systemDataCard()`, `exportReport()`, an inline confirmation that says the report lists file paths, and an inspector for "Review all safe". Labels still come from `shownLabel`, unchanged.
- The picked set (`stMulti`), "Show all" and the export prompt are in memory, not in the URL hash, so the hash allow-lists did not change.
- `ST_RULE_TEXT` moved into the new block so the report can use it. The floor test's `nameAttrs` count went from 5 to 6.
- Indexes always read "unavailable": the scan reads no Spotlight index. "Other" is every measured section that is not caches, logs, downloads, mail or applications (developer tools).
- The clean-ups list is a card list, not a table, so it has no `TABLE_META`/`EXPORT` entry.

Checked in a real browser (Chrome, scratch HOME, window not resized): list ranking and chips, Review all safe sum (34.0 MB = the two safe rows), system data rows with the snapshot reason, the export prompt, and `reportHtml` run in the page (escaped name, no script, no remote address).

Review round 1 (fixed in the second work commit):

- Spec: the list header now shows the safe subtotal beside the total; `safeSubtotalText` and the inspector both use `sumText(sumSizes(...))`, with a test over plain, lower-bound, unavailable and no-safe sets.
- Focus moves into the export panel when it opens and back to the Export button on Cancel or Download; "Review all safe" is announced. Review all safe resets the section, label and search filters so the picked rows are in the table; changing any of them, or leaving the page, drops the selection, "Show all" and the export prompt.
- Both lists are `ul`/`li` with a visible column header row and a full `aria-label` on each Review button (name, label, size).
- The report shows paths as `~/...`, flattens newlines and control characters to `\n`, `\t`, `\xNN`, and the prompt says so. Tests cover a newline, a single quote and hostile filter text.

Also checked in Chrome on a scratch HOME: focus order, announcement text, 10,000 rows (50 shown, Show all draws all 10,000), and the report (built from the same scan with Node, viewed over http) matches the page's numbers.

Pending:

- [ ] The browser's own download did not land on disk in the automated session and `file://` is blocked for the extension, so the file was viewed over local http, not opened offline from disk.
- [ ] A scan of a real developer home was not looked at.
- [ ] `/code-review` (second round) and `/writing-review` not run.
- [ ] Narrow-window layout of the new lists not checked.

**Moved to `done/` on 2026-11-27**, after PR #54 merged; the move was missed in that PR. Boxes still unchecked above are checks this record lists as not run or not verified; they were not ticked on the move.
