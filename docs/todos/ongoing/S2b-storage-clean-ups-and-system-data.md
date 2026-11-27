# S2b: Storage page: suggested clean-ups and Inside System Data

| Field | Value |
|---|---|
| ID | `S2b` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D4, D9 |
| Depends on | S2a |
| ADRs | None |
| Branch | `feat/s2b-storage-clean-ups` |
| PR title | `feat(web): add suggested clean-ups and Inside System Data to Storage` |
| Size | M (200-400) |
| PR | — |

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
