# S2a: Storage page: status strip, treemap, table and inspector

| Field | Value |
|---|---|
| ID | `S2a` |
| Status | `ready` |
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
- The scan is partial on a normal developer home: S1 reached about 270 of 305 locations in 20 s. See S1b.
- The page must say so. Show a Partial banner while `complete` is false, and "at least" before a total whose `lower_bound` is true.
- Show "measured N min ago" per row and "not yet scanned" rows for `not_reached`. Never show a final treemap or a "safe to remove" total while `complete` is false.
- B7 builds the capacity bar; reuse it, do not copy it.
- `web/index.html` conventions: `h()` helper, `rec()` state, `sortHeader`, `paginate`, bundled fonts, no remote assets.

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
| `web/index.html` | nav entry, `storage()` view, page CSS |

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
