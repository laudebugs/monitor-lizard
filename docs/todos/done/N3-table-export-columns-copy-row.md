# N3: Table export, column chooser and copy row

| Field | Value |
|---|---|
| ID | `N3` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D14 |
| Depends on | None |
| ADRs | None |
| Branch | `feat/n3-table-export-columns-copy-row` |
| PR title | `feat(web): export, choose columns and copy a row on every table` |
| Size | M (200-400) |
| PR | — |

## Goal

Give every table the same three tools: export what is on screen as CSV or JSON, choose which columns show, and copy one row as JSON. This makes the data parsable outside the dashboard.

## Context

- C1 added `sortRows`, `sortHeader` and filters in `web/index.html`; they are the one place to hook into.
- A commenter on the disk-tool HN thread asked for shareable reports (read directly; one comment).

## Scope

**In scope:**

- Export uses the rows after filters and sort, all pages (not only the current page of pagination), with the visible columns and a header row.
- CSV is RFC 4180 (quotes doubled, CRLF), UTF-8; a cell starting with `=`, `+`, `-` or `@` gets a leading `'` so spreadsheets do not run it as a formula.
- JSON is an array of objects keyed by column id, with timestamps as epoch seconds and an ISO string.
- Column chooser stores the choice in `localStorage` per table.
- Copy row JSON through the clipboard with a fallback that selects the text.
- Download starts from a click through a Blob URL; no data leaves the machine.

**Out of scope** (name the todo that covers it):

- HTML or PDF reports: later. Export of the Storage treemap: S2b.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `exportRows`, `toCsv`, `toJson`, chooser, copy button in `sortHeader` and inspectors |

## Steps

- [x] Write `toCsv` tests first, including formula and quote cases.
- [x] Hook the toolbar into `sortHeader` so every table gets it without per-page code.
- [x] Check the exported text parses correctly: inspect the Blob text in the browser and read generated CSV with Python's `csv` module (a download cannot be opened in Numbers from the browser sandbox).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Cell with comma, quote or newline | Quoted and escaped |
| Cell starting with `=SUM(1,1)` | Prefixed with `'` |
| Zero rows | Header only, and the button says "Nothing to export" |
| 50,000 rows | Completes without freezing the page for more than a second |
| All columns hidden | The last one stays |
| Non-ASCII app names | Preserved in UTF-8 |

## Acceptance criteria

- Exporting a filtered, sorted table gives the same rows in the same order as on screen.
- A cell beginning with `=` never starts a formula when opened in a spreadsheet.

## Tests

Logic tests, 10 minimum. Download and open checked by hand in a real browser.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; README feature list.

## Risks and open questions

- Exports of Network and Persistence reveal habits to whoever receives the file; the export button states what it contains.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `sortHeader` returns an export bar plus the header, so every table gets CSV and JSON export, a column chooser and a Copy button on each row. Apps also has "Copy as JSON" in its inspector. The pure functions sit in an `export` block that `tests/web/export.test.mjs` loads.
- **How:** `paginate` records the filtered, sorted rows of each table. One `EXPORT` map holds the text each column exports, and one `TABLE_META` holds file name, description and time columns. Hidden columns and Copy buttons are applied to the drawn table after each render, and only when its row and cell counts match its data.
- **Review fixes:** a table that does not line up is skipped with one console warning. A "none" dash is exported empty. The note says a file stays on this Mac only until it is shared, explains the leading apostrophe, and warns that commands can hold secrets. Tables not on the current page are dropped from memory on each render. The column menu now takes keyboard focus when opened.
- **Tests:** 26 Node logic tests for export (quotes, commas, newlines, formula starts, zero rows, all columns hidden, damaged storage, non-ASCII, 50,000 rows, alignment check, pruning, `__proto__` column). `make check` and `make test` pass.

- **Browser (Chrome, macOS, 2026-11-26):** the toolbar was seen on Apps, Sudo, Authorization, Listeners, Persistence, Network, New hosts and Actions. A script compared drawn rows, cells and Copy buttons with the page rows on every table, normal and with the Apps inspector open; all matched. Sudo, New hosts and Actions had no stored data, so those three used rows injected into page memory. Copy on the first, fourth and last row of Apps, Listeners, Persistence and Network matched the row shown.
- **Browser, other:** a filtered, sorted Network export held every row in screen order. The column choice survived a reload. Keyboard: Tab to Columns, Enter opens with focus on the first box, Space toggles, Escape closes and returns focus. Light theme and a 400 px frame were checked.
- **Parsing check:** Python's `csv` module read a CSV made from fixtures and from 32 real startup items; every cell equalled the expected text, with the apostrophe guard applied.
- **Deviations:** no byte order mark, so very old Excel may misread non-ASCII text. File names carry no product name. With zero rows both buttons read "Nothing to export" and still save a header-only file.
- **Open:** opening a file in Numbers was not done. `/writing-review` is pending. `web/index.html` is now about 1,060 lines against the 800-line module budget (it was 852 on main).
- **Open:** focus is lost on Copy and export buttons after each re-render; the page rebuilds the whole DOM on every poll, which was already so.
- **Open:** column data still lives in three places that a column change must touch: the `sortHeader` call with `PAGE_SORTS`, the `EXPORT` line, and `TABLE_META.time` for a time column. Merging `EXPORT` into `TABLE_META` needs its alignment test reworked, so it is left as a follow-up.
