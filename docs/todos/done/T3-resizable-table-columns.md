# T3: Resizable (expandable) table columns

| Field | Value |
|---|---|
| ID | `T3` |
| Status | `done` |
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

- [x] Write the pure resize math first (given a track list, a dragged column's id and a delta, the new track list, clamped to each column's own minimum and the row's floor) as its own tested function.
- [x] Add the handle to `sortHeader`'s header cells and wire pointer drag to that function.
- [x] Persist and restore widths per table, and add the keyboard and double-click-to-reset paths.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `web/table.js`: a new `colresize` pure-logic block (`isFlexTrack`, `applyColumnWidths`, `clampDrag`); `widthsOf`/`setWidth` persist a column's chosen px width per table under its own `table-colw:` localStorage key, parallel to `hiddenOf`/`setHidden`'s `table-cols:`. `sortHeader` draws a `.colgrip` (`role="separator"`, focusable) on the right edge of every column whose track is flexible (`fr`); a fixed-width column (an icon or action column) gets none. `startColResize` drags the grip: it live-patches every row's `gridTemplateColumns` per animation frame (never a full `render()` mid-drag, matching the existing drawer-resize comment's own reasoning) and commits with one `render()` on release. `colHandleKeys` gives the same resize in fixed 16px steps via the arrow keys, and a double-click resets the column to its default width. `decorateTables`'s two `setFloor` calls and its row-track loop now run every track list through `applyColumnWidths` first, so a widened column's new size becomes its own floor minimum (that is the point of widening a column, not just letting it shrink back once the window narrows), and a stale stored id (a since-renamed or removed column) is simply inert. `web/app.css`: `.cwrap`/`.colgrip` draw the handle, invisible until hover or focus, in the same visual language as the existing `.handle` drawer resizer.
- **Tests:** `tests/web/table-colresize.test.mjs` (new), 13 cases: `isFlexTrack`, `applyColumnWidths` (widen, a fixed column is a no-op, clamped at the minimum, a stale id is ignored, rounds a fractional width), `clampDrag`, that `decorateTables` and the drag/keyboard handlers actually call these functions rather than duplicating the math inline, and that the grip is keyboard-reachable and double-click-resettable. All 364 web tests and 485 Rust tests pass; `make check` is clean.
- **Real run:** built the release bundle, served it with a scratch `HOME`, and drove it through the Claude in Chrome extension (the extension's own `left_click_drag` did not reproduce the drag correctly in this environment -- it reads as a single pointer jump rather than a mousedown/mousemove/mouseup sequence -- so the drag, keyboard and double-click paths were exercised with real `MouseEvent`/`KeyboardEvent` dispatch instead, which is the actual DOM event sequence a browser's own pointer drag produces). Verified on the Apps table: dragging the App column's grip 150px wider actually widens the name cell (120px -> 150px); the width survives a page reload; ArrowRight on the focused grip steps it by 16px; double-click resets it to 120px and clears its stored entry. At a 400px same-origin frame, widening a column to 270px keeps the document's own `scrollWidth` at 400 -- the table scrolls inside its card instead of the page itself overflowing, confirming the "existing floor/scroll behavior still applies" edge case. Re-verified the (pre-existing, now-shared) drawer handle still resizes the Apps inspector panel after the `beginDrag` extraction below.
- **Review (2026-11-27):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: `docs/agents/layout.md` was not updated for the new `colresize` block (added); `startColResize` reproduced `startDrawerResize`'s mousemove/mouseup/blur wiring and leak-guard near-verbatim -- extracted into one shared `beginDrag(onMove, onUp)` helper both now call, with a single `activeDragEnd` guard (only one drag is ever in progress at a time, so one guard correctly covers both instead of two separate ones). Spec findings fixed: the delta-clamp math (`Math.max(minW, startW + delta)`) lived inline in two event handlers, untested, even though Step 1 asked for "a delta ... clamped to each column's own minimum" as its own tested function -- extracted as `clampDrag` and used by both the drag and the keyboard step, with its own tests. Judgement call not adopted, with reason: `widthsOf`/`setWidth` were flagged as duplicating `hiddenOf`/`setHidden`'s lazy-memoized-localStorage-cache shape; left separate because `hiddenFrom` (used by `hiddenOf`) has its own established signature and dedicated tests in `tests/web/export.test.mjs` for a differently-shaped value (a capped array of strings) than a plain width map, and unifying would mean changing that already-tested function's contract for a handful of lines saved.
- **Deviation:** the keyboard handler is bound to the grip itself (a new, separately-tabbable control), not to the existing sort-button header cell the Scope's looser wording ("with a header cell focused") suggested. This matches the established precedent exactly: the drawer's own resize handle is likewise `role="separator"`, its own tab stop, with its own Arrow-key handling, separate from any other control -- the same accessible pattern native column-header splitters (Finder, Explorer) use. Accepted as the right call over literal-reading the Scope line, at the cost of one extra tab stop per resizable column.
- **Open:** none beyond the pre-existing "Design gap" the plan already tracks for the handle's exact visual treatment (Risks item 1), which the owner can redirect once seen.
- **CI:** pending on the pull request.
