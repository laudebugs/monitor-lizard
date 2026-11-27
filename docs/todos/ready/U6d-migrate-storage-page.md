# U6d: Migrate the Storage page

| Field | Value |
|---|---|
| ID | `U6d` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U5, U6c, S2a, S2b, S3b |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6d-storage-page` |
| PR title | `refactor(web): run the Storage page on the Preact path` |
| Size | L (over 400: split into tiles/treemap, table, inspector) |
| PR | — |

## Goal

Storage is the largest page (178 `h` calls, 518 lines, treemap, inspector drawer, clean-ups, selection, the Trash flow). Move it last, when every other pattern has already shipped on the Preact path.

## Context

- Spike: Storage page (240 items, about 1,070 elements) full rebuild 11.4 ms vs update 2.0 ms; one DOM difference, the capacity chart (fixed by U3).
- `stFocus`, `tileKeys`, `stMulti` and the selection state; `S2a`, `S2b`, `S3b` are `ongoing`.

## Scope

**In scope:**

- Treemap tile keyboard navigation (arrow keys, `data-tile`), the table with checkboxes and `stMulti`, the inspector, clean-ups, the capacity card, the History strip, and the Trash flow from U5, all on the Preact path; delete `stFocus`.

**Out of scope** (name the todo that covers it):

- Deleting the old path (U7).

## Target files

| File | Change |
|---|---|
| `web/page-storage.js`, `web/page-storage-logic.js` | keys (item ids), neutral helpers; logic blocks untouched |
| `tests/web/storage-*.test.mjs` | markup cases on the stand-in |

## Steps

- [ ] Run the full flow in a browser against a scratch `HOME` with fixture data: scan, filter by tile, select several rows, Review all safe, Move to Trash preview, cancel, rescan.
- [ ] Never run the real Trash flow against the real home folder; use only preview or a scratch folder.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| 240+ items | list capped by paging; update under 5 ms |
| Selection kept through a rescan | by item id, not index |
| Treemap arrow keys then a poll | focus stays on the same tile |
| Scan refused (`HOME` check) | the refusal text, no chart |
| Capacity chart with no data | empty state |
| Table export with a tile filter | exports the filtered rows |

## Acceptance criteria

- Storage matches the old path in DOM and behaviour; `stFocus` unused on the Preact path; the timings are recorded.

## Tests

Existing storage tests stay green; browser checks recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` Storage test rows if blocks move.

## Risks and open questions

- Largest diff of the series; split if it passes 400 lines.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR.
- **Why the page works at every step:** Old path remains the default until U7.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
