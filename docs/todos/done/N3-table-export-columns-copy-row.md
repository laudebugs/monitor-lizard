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
- [ ] Check a download opens correctly in Numbers.

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
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `sortHeader` now returns an export bar plus the header, so every table gets CSV and JSON export, a column chooser and a Copy button on each row. The pure functions sit in an `export` block that `tests/web/export.test.mjs` loads. Apps also has "Copy as JSON" in its inspector.
- **How:** `paginate` records the filtered, sorted rows of each table. One `EXPORT` map holds the text each column exports. Hidden columns and the row buttons are applied to the drawn table after each render.
- **Tests:** 19 logic tests, among them quotes, commas, newlines, formula starts, zero rows, all columns hidden, damaged stored choices, non-ASCII text and 50,000 rows. `make check` and `make test` pass.
- **Browser (Chrome, macOS, 2026-11-26):** toolbar seen on Apps, Listeners and Network (dark and light); other tables only counted. A filtered, sorted Network export held all 70 rows in screen order. Column choice survived a reload. A name starting with `=` was written as `'=...`. Zero rows gave a header only. Copy fell back to a selected text box when the clipboard was refused. A 400 px frame scrolls the table sideways.
- **Deviations:** the CSV has no byte order mark, so very old Excel may misread non-ASCII text. File names carry no product name (`network-2026-11-26.csv`). The "-" placeholder is exported as empty, since a leading "-" would become text. With zero rows both buttons read "Nothing to export" and still save a header-only file.
- **Open:** opening a file in Numbers was not checked. Keyboard use of the column menu was not exercised in a browser. `/code-review` and `/writing-review` are pending.
