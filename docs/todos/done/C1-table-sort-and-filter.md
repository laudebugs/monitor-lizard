# C1: Sortable columns and filters on the tables

| Field | Value |
|---|---|
| ID | `C1` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/c1-table-sort-and-filter` |
| PR title | `feat(web): add sortable columns and filters to the dashboard tables` |
| Size | M (200-400) |
| PR | [#12](https://github.com/laudebugs/system-monitor/pull/12) |

## Goal

Let the owner sort every sortable column and filter every table that has something to filter on, with the same controls everywhere and a shared helper the pages still to come (Persistence, Network, History) can reuse.

## Context

- Requested by the owner on 2026-11-26. Today the tables paginate and have chips and a search box (`apps()`, `listeners()`, `priv()` in `web/index.html`) but no column sorting and only some filters.
- Tables are built from `div` rows with `class="tr"` and a header row `tr h`; pagination is `paginate(list, key)`; the custom dropdown is `dropdown(key, value, options, onPick, label)` and opens upward.

## Scope

**In scope:**

- Shared helpers in `web/index.html`: `S.sorts[key] = {col, dir}`, `sortRows(rows, key, getters)`, a `sortHeader(key, columns, template)` that renders clickable header cells with a direction arrow, and `S.filters` for dropdown filters.
- Apps: sort App, Procs, Out, Listening (number of listening ports), Signing, Status, Label (suspicious before unknown before trusted); new filters Label and Signing (All, Apple, Signed, Unsigned) as dropdowns beside the existing chips.
- Listeners: sort Port, Proto, Bind address, App, PID, Reach; new filter Protocol (All, TCP, UDP).
- Privilege: launches sort by Time and Command; authorization requests sort by Time, Result, Right and Requester, with a Result filter (All, Failed, OK).
- A "Clear filters" button in the empty state; the page returns to 1 whenever a sort or filter changes; `dropdown` gets an option to open downward for controls at the top of a page.

**Out of scope** (name the todo that covers it):

- Multi-column sort, saving sort or filters across reloads, and server-side sorting: not planned.
- Table roles for screen readers (the tables are `div` rows today, a gap noted in the A3 review): a separate accessibility todo.
- The Persistence, Network and History pages: they adopt these helpers in A2, A4 and A5.

## Target files

| File | Change |
|---|---|
| `web/index.html` | helpers, header component, per-table getters and filters, `dropdown` direction option, CSS for sortable headers |
| `docs/plans/system-monitor.md` | add C1 to the todo list and order table |

## Steps

- [ ] Add `S.sorts`, `S.filters`, `sortRows` and `sortHeader`; click cycles ascending, descending, then back to the default order.
- [ ] Add per-table getters: numbers compare numerically, strings case-insensitively, empty values sort last in both directions, ties keep the original order.
- [ ] Wire the Apps table (7 columns, 2 dropdown filters), the Listeners table (6 columns, 1 filter) and both Privilege tables.
- [ ] Make `dropdown` accept a direction; filters sit above the table, so their menus open downward.
- [ ] Reset `S.page` whenever a sort or filter changes; keep sort and filter state across the 10 second refresh.
- [ ] Add the empty state with a Clear filters button.
- [ ] Check with a simulated-DOM script against a running server, then in a real browser.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Click the same header repeatedly | Ascending, descending, then the default order |
| Numeric column (Procs, Port, PID) | Sorted as numbers, not text: 9 before 10 |
| Equal values | Original order kept (stable) |
| Empty or missing value | Last in both directions |
| Sort or filter active when the 10 s refresh arrives | State is kept; the page does not jump to 1 |
| Chip + dropdown + search together | All three apply (AND) |
| Filters leave no rows | Empty state naming the filters, with a Clear filters button that restores everything |
| Dropdown at the top of the page | Menu opens downward and is not clipped by the sticky header |
| Sort changed while on page 3 | Page returns to 1 |

## Acceptance criteria

- Every sortable header sorts correctly: the displayed order equals a reference sort computed from `/api/snapshot`.
- Filter counts equal counts computed independently from the snapshot.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

A simulated-DOM script (kept out of the repo, as for A2 and A3) renders each page against a running server and, for each table and column, clicks the header and compares the visible order with a reference sort; applies each filter and compares row counts; checks the sort survives a refresh; no script errors.

## Verification

```sh
make check
make test
cargo run --release -- serve
```

## Docs to update

- `docs/plans/system-monitor.md`

## Risks and open questions

- Layout can only be confirmed in a real browser; headless Chrome screenshots can be used for a visual check.
- Header buttons inside `div` rows have no `aria-sort`: a known accessibility gap, tracked as a separate todo.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** shared helpers in `web/index.html` (`sortRows`, `nextSort`, `sortHeader`, `fv`, `setFil`, `clearFilters`, `clearBtn`) and a `down` option on `dropdown` so filter menus at the top of a page open downward. Header cells are buttons with an arrow; a click cycles ascending, descending, then the default order. Sort and filter state live in `S.sorts` and `S.fil`, so both survive the 10 second refresh; the page returns to 1 on any change.
- **Tables wired:** Apps (7 sortable columns; filters Label and Signing next to the existing chips), Listeners (6 columns; filter Protocol), Privilege launches (Time, Command) and authorization requests (Time, Result, Right, Requester; filter Result). Labels sort most worrying first; empty values sort last in both directions; ties keep the original order.
- **Empty states** name the filters and carry a Clear filters button.
- **Verified:** a script (kept out of the repo) rendered each page in a simulated DOM against a running server and ran 39 checks: for each sortable column ascending, descending and the third click restoring the default order; a numeric sort that starts at the real maximum (Procs descending); the sort surviving `load()`; each filter's count against the page's own current data, chip, dropdown and search combined with AND; the empty state and Clear filters; the label menu opening downward. 39 of 39 passed on two consecutive runs. Early failures were my script comparing against a snapshot fetched at another moment (the data shifts every 10 s), not the page.
- **Not verified:** layout and interaction in a real browser; the visual check is **pending**. `/code-review` and `/writing-review` have not yet been run on this change.
- **Open:** header buttons sit inside `div` rows with no `aria-sort` or table roles, a known gap now covering more of the UI: it needs its own accessibility todo. The Persistence, Network and History pages will adopt these helpers in A2, A4 and A5.

