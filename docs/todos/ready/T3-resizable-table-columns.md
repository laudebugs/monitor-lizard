# T3: Resizable (expandable) table columns

| Field | Value |
|---|---|
| ID | `T3` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D14 |
| Depends on | T1b, N3 |
| ADRs | None |
| Branch | `feat/t3-resizable-table-columns` |
| PR title | `feat(web): let a column be dragged wider on every table` |
| Size | M (200-400) |
| PR | — |

## Goal

A user can drag a column's right edge to widen it, for example to read a truncated name in full without opening the inspector. This works on every table that already uses the shared `sortHeader`/`decorateTables` mechanism, and the choice is remembered per table the same way N3 remembers which columns are shown.

## Context

- N3 already lets a user choose which columns are *shown*; nothing today lets a column be made *wider*. A narrow floor-collapsed column (see `docs/bugs/network-table-columns-collapse-at-narrow-width/`) or an ellipsis-truncated name cell (`nameAttrs`) can only be read via its tooltip or the row's inspector today.
- `sortHeader`/`decorateTables`/`setFloor` in `web/index.html` are the one shared place every table's header and column widths already go through; this is the one place a drag handle needs to be added, not a per-page change.
- No design for a drag-handle's exact look exists in the owner's design canvas yet; Steps below pick a minimal, consistent treatment (a thin edge the pointer can grab, highlighted on hover) rather than waiting on one.

## Scope

**In scope:**

- A draggable handle on the right edge of every resizable header cell (the same cells `sortHeader` already draws). Dragging it widens that one column's track and shrinks the available space for flexible columns, never pushing the row wider than its card; the existing floor/scroll behavior still applies beyond that.
- The chosen width is stored in `localStorage` per table and column id, the same key scheme N3 uses for shown/hidden columns, so it survives a reload.
- Double-clicking a handle resets that column to its default width.
- Keyboard equivalent: with a header cell focused, Left/Right (or a modified arrow) resizes it in fixed steps, for a user who cannot drag.

**Out of scope** (name the todo that covers it):

- Reordering columns: not asked for.
- A per-user saved "layout" beyond column widths and visibility (already N3's scope): none planned.

## Target files

| File | Change |
|---|---|
| `web/index.html` (or `web/table.js` once T2 merges) | Drag handle, resize math, persisted widths, keyboard resize |

## Steps

- [ ] Write the pure resize math first (given a track list, a dragged column's id and a delta, the new track list, clamped to each column's own minimum and the row's floor) as its own tested function.
- [ ] Add the handle to `sortHeader`'s header cells and wire pointer drag to that function.
- [ ] Persist and restore widths per table, and add the keyboard and double-click-to-reset paths.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Dragging a column narrower than its own minimum | Clamped at the minimum, never collapses past it |
| Dragging the last column | Nothing to shrink into; the row width grows and the existing horizontal-scroll floor takes over |
| A table with only hidden-by-N3 columns remaining visible | Resize still works on whatever is currently shown |
| Stored width from a table whose columns changed since (a renamed or removed column id) | Ignored, falls back to the default width, never applied to the wrong column |

## Acceptance criteria

- Every table that uses `sortHeader` can have at least one of its columns widened and the width survives a reload.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending).

## Tests

jsdom smoke test for the resize-math pure function, 4 cases minimum.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- No approved design exists yet for the drag handle's exact visual treatment; Steps pick one consistent with the existing table chrome, and the owner can redirect it once seen.
- Interacts with the existing floor/column-hiding system (`setFloor`, `decorateTables`): a column widened by hand and then auto-hidden by the floor at a narrower viewport needs a defined order (floor wins, so a hidden column's stored width is inert until it is shown again) -- confirm this in Steps rather than leaving it implicit.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
