# U6d: Migrate the Storage page

| Field | Value |
|---|---|
| ID | `U6d` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U5, U6c |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6d-storage-page` |
| PR title | `refactor(web): run the Storage page on the Preact renderer` |
| Size | L (over 400: split into tiles and treemap, table, inspector) |
| PR | — |

## Goal

Storage is the largest page (178 `h(` calls, 518 lines, treemap, inspector drawer, clean-ups, selection, the Trash flow). Move it last, when every other pattern has already shipped on the Preact renderer.

## Context

- The code from S2a (#51), S2b (#54) and S3b (#69) is on `main`; their todo files stay in `ongoing/` for owner-run checks and are not touched. Spike: Storage page differs from the old renderer only in the capacity chart (fixed by U3).
- `stFocus`, `tileKeys`, `stMulti` and the selection state.

## Scope

**In scope:**

- Treemap tile keyboard navigation (arrow keys, `data-tile`), the table with checkboxes and `stMulti`, the inspector, clean-ups, the capacity card, the History strip and the Trash flow from U5, all on the Preact renderer; delete `stFocus`; keys from item ids; escaping tests for every folder name and server text.

**Out of scope** (name the todo that covers it):

- Deleting the old renderer (U7).

## Target files

| File | Change |
|---|---|
| `web/page-storage.js`, `web/page-storage-logic.js` | keys, neutral helpers; logic blocks untouched |
| `tests/web/storage-*.test.mjs` | markup cases on the vnode stand-in |

## Steps

- [ ] With a scratch `HOME` (never the real one) and fixture data: scan, filter by tile, select several rows, Review all safe, Move to Trash preview, cancel, rescan.
- [ ] Never run a real move against the real home folder; use preview, or a scratch folder only.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| 240+ items | paging caps the list |
| Selection kept through a rescan | by item id, not index |
| Treemap arrow keys then a poll | focus stays on the same tile |
| Scan refused (`HOME` check) | the refusal text, no chart |
| Capacity chart with no data | the empty state |
| Table export with a tile filter | exports the filtered rows |
| A hostile folder name | escaped by `visible()` everywhere it appears |

## Acceptance criteria

- Storage matches the old renderer in DOM and behaviour; `stFocus` is unused on the Preact renderer.
- **Maintainability (the reason for this plan):** each page or component moved deletes the workarounds it needed (a `data-req` or `stFocus`-style focus restore, a `quiet()` guard, a dataset-cached state) and the PR description records before and after counts of those workarounds and of lines in the files touched. Logic that a page owns (labels, rows, text) is tested in Node against the vnode stand-in, with no browser needed; only behaviour that needs a real browser (focus, `inert`, scroll) is checked by hand.

## Tests

Existing storage tests stay green; markup, key and escaping cases; browser checks recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/storage-*.test.mjs
```

## Docs to update

`docs/agents/layout.md` Storage test rows if blocks move.

## Risks and open questions

- The largest diff of the series; split if it passes 400 lines.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** The default is the old renderer until U7.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
