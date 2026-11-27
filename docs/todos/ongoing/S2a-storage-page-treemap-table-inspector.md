# S2a: Storage page: status strip, treemap, table and inspector

| Field | Value |
|---|---|
| ID | `S2a` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D1, D9 |
| Depends on | S1, B7, T1a |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s2a-storage-page` |
| PR title | `feat(web): add the Storage page with treemap, table and inspector` |
| Size | M (200-400) |
| PR | — |

## Goal

Add the Storage nav item and the first half of the page from design direction D: a status strip, a treemap of sections, a folder table with a share-of-disk bar and an inspector. Removal buttons are present but disabled until S3b. The clean-ups list and "Inside System Data" are S2b.

## Context

- Design canvas "Sys Monitor Redesign Directions" (2026-11-26), boards D and "Research and directions".
- The scan is partial on a normal developer home: S1 reached about 270 of 305 locations in 20 s. S1b makes it converge: the server's background scan runs 120 s and merges into what earlier scans learned, so the page sees a result that fills in over several scans. The page must say where it stands.
- Show a Partial banner while `complete` is false, and "at least" before a total whose `lower_bound` is true.
- Show a row's age from `measured_at` ("measured N min ago"). When `reused` is true, say the size comes from an earlier scan. When `stale` is true, the size is an earlier complete value that the latest scan could not confirm: show it as "at least" with its age. Show "not yet scanned" rows for `not_reached`.
- Never show a final treemap or a "safe to remove" total while `complete` is false. Rows with `complete` false cannot be moved to the Trash.
- Expect `scanned_at` to be newer than most rows' `measured_at`: a scan reuses locations whose top-level modification time has not changed, and walks the two oldest of the largest again each scan.
- B7 builds the capacity bar; reuse it, do not copy it.
- Conventions in `web/*.js`: `h()` helper, `rec()` state, `sortHeader`, `paginate`, bundled fonts, no remote assets.

## Scope

**In scope:**

- Nav item "Storage"; polling `GET /api/storage` once on open and on Rescan; a scanning indicator and a stale marker.
- Treemap built from flex columns; each tile is a `<button>`; the selected tile filters the table and sets the inspector.
- Labels `SAFE`, `REVIEW`, `YOURS` as chips; unavailable values show their reason, never 0.
- Use the shared tokens, buttons and chips from T1a; add no page-specific button styles (plan D9).
- Keyboard: Tab reaches every tile and row; arrow keys move between tiles.

**Out of scope** (name the todo that covers it):

- Suggested clean-ups, Inside System Data and the HTML report: S2b. Removal: S3a and S3b. Forecast and weekly growth: S4. Disk writes panel: S5.

## Target files

| File | Change |
|---|---|
| `web/page-storage.js`, `web/page-storage-logic.js`, `web/core.js`, `web/table.js`, `web/app.css` | nav entry (`shell.js`), `storage()` view (`page-storage.js`), `storage-page` pure block (`page-storage-logic.js`), url-state entries (`core.js`), TABLE_META and EXPORT entries, page CSS |
| `tests/web/storage-page.test.mjs` | (new) logic tests for the pure block and the rules.rs cross-check |
| `tests/web/url-state.test.mjs` | Storage filter, tile, item, search and sort |
| `tests/web/page-shell.test.mjs` | Storage nav group and verdict title |
| `tests/web/export.test.mjs` | table count 8 to 9 |
| `tests/web/table-floor.test.mjs` | name-cell count 4 to 5 |
| `docs/agents/layout.md` | the new `storage-page` test block |

## Steps

- [ ] Add `['storage', 'Storage', <icon path>]` to `NAV` and a `storage()` renderer.
- [ ] Build the treemap with `flex-grow` set to each section's bytes; minimum tile size so small sections stay readable and keep a label.
- [ ] Table and inspector reuse the existing sort and pagination helpers.
- [ ] Load the page in a real browser at 1360 px and at 400 px; record what was seen.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `/api/storage` is empty (first scan running) | Skeleton rows and "Scanning, 12 s so far" |
| A section is unavailable | Tile shown hatched with the reason |
| A section under 1% of the disk | Merged into "Other" tile with its list on click |
| 400 px width | Treemap stacks as a list of bars; table scrolls in its own box |
| `prefers-reduced-motion` | No animated fills |

## Acceptance criteria

- Opening Storage on a real Mac shows the treemap within one scan.
- The sum of tile sizes equals the used total shown in the strip, within 1 GB.
- Every control is reachable by keyboard and has a visible focus state.

## Tests

Logic tests for the treemap sizing and the "Other" merge in the existing simulated DOM harness (6 cases). Layout is checked by hand in a real browser and recorded; mark it pending if no browser is available.

## Verification

```sh
make check
make test
cargo run --release -- serve   # open the Storage page at 1360 px and 400 px
```

## Docs to update

Plan status; `README.md` page list if it names pages.

## Risks and open questions

- Every new page lands in the same file; watch the size budget in `docs/guidelines/maintainability.md`.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Status (2026-11-26)

Built and checked in a real browser on a synthetic home. The todo stays in `ongoing/` because these are open:

- [ ] `/code-review` and `/writing-review` are not run (the owner's session does them).
- [ ] A scan of a real developer home has not been looked at.
- [ ] A physical keyboard has not been used; arrow keys were sent as synthetic events.
- [ ] The owner's window is 1352 px wide, so 1360 px is not available; the widest check is recorded in the PR.
- [x] The Performance page does not filter by the search box; typing there no longer jumps to Apps (closed by the global search, N2).

## Notes

- The tiles add up to the used total through a "Not measured" strip (used minus measured). The scan reads known folders only.
- That strip reads "about" while any section is unavailable, a lower bound or unfinished, and says it is not free space.
- Per-row "measured N min ago" is the scan time. The API has no per-row timestamp.
- The server answers a Rescan inside its minute with 200 and the old result, never 429. A 429 branch was written first and was dead code, so it is removed.
- SAFE is shown only for the server label `safe`, a rule on the page's safe list, and an absolute path under `/Users/<name>/`. A test reads `src/storage/rules.rs` and fails if the lists differ.
- The Folder column shows the home-relative path and cuts only the front, so the last two segments always show. The title and the export keep the full path.
