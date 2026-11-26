# C2: Expand truncated lists and a resizable app detail panel

| Field | Value |
|---|---|
| ID | `C2` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/c2-expand-processes-resizable-drawer` |
| PR title | `feat(web): show all processes and connections, and let the detail panel be resized by dragging` |
| Size | S (under 200) |
| PR | — |

## Goal

The app detail panel truncates processes at 12 and connections at 20 with a static "+N more" line. Let the owner expand each list to see every row, and let them drag the boundary between the table and the panel to resize both, with the width remembered.

## Context

- Requested by the owner on 2026-11-26, with a screenshot of a truncated process list.
- The panel is `drawer()` in web/index.html, in a flex row with the Apps table; today it has a fixed max-width of 460px and the table flex-grows around it.

## Scope

**In scope:**

- "+N more" becomes a button that expands the list in place; an expanded list gets "Show fewer".
- A drag handle on the left edge of the panel resizes it (and the table shrinks to match) between a minimum and maximum width; the width is remembered in localStorage and applied on the next open.
- Works with the mouse; the handle is reachable by keyboard with arrow keys as a fallback.

**Out of scope** (name the todo that covers it):

- Resizing any other panel: not planned.
- Virtualizing very long lists: out of scope unless real data shows it is slow.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `drawer()` expand/collapse state, a resize handle and width state, CSS for the handle |

## Steps

- [ ] Add `S.expand = {procs:false, conns:false}` per drawer section; the button toggles it and shows all rows when true.
- [ ] Add a draggable handle: on pointerdown, track pointer position; on pointermove, compute a new width clamped to [320, 720] and apply it to the drawer's inline width; on pointerup, save it to localStorage.
- [ ] Give the handle `role="separator"`, `aria-orientation="vertical"`, and ArrowLeft/ArrowRight handlers that resize in 16px steps.
- [ ] Verify with a simulated-DOM script against a running server: expand shows every row, collapse returns to 12/20, dragging changes the width and it survives a reload.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| App with 0 extra processes or connections | No Show all button appears |
| Dragging past the minimum or maximum | Clamped, does not go further |
| Keyboard resize | ArrowLeft/ArrowRight on the focused handle change the width |
| Switching to a different app while expanded | Expand state resets for the new app |
| Narrow window (below 1100px) | The drawer already stacks below the table; the handle is hidden there, since there is nothing to resize against |

## Acceptance criteria

- Expanding shows every process and every connection.
- Dragging the handle resizes the panel and the table together, within bounds, and the width persists after a reload.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

A simulated-DOM script (kept out of the repo, as in prior UI todos): expand/collapse row counts, and a resize via pointer events checked against the clamped bounds.

## Verification

```sh
make check
make test
cargo run --release -- serve
```

## Docs to update

- `docs/plans/system-monitor.md`

## Risks and open questions

- Layout can only be confirmed in a real browser; a headless Chrome screenshot can check the static layout.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
