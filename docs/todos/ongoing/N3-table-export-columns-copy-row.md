# N3: Table export, column chooser and copy row

| Field | Value |
|---|---|
| ID | `N3` |
| Status | `ongoing` |
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

- [ ] Write `toCsv` tests first, including formula and quote cases.
- [ ] Hook the toolbar into `sortHeader` so every table gets it without per-page code.
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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
