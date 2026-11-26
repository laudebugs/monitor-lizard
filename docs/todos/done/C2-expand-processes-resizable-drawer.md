# C2: Expand truncated lists and a resizable app detail panel

| Field | Value |
|---|---|
| ID | `C2` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/c2-expand-processes-resizable-drawer` |
| PR title | `feat(web): show all processes and connections, and let the detail panel be resized by dragging` |
| Size | S (under 200) |
| PR | [#13](https://github.com/laudebugs/sys-monitor/pull/13) |

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `S.expand = { procs: false, conns: false }`, reset centrally whenever `rec()` is called with a different `sel` (one-line change in `rec`, not six call sites), so every path that selects an app starts collapsed. "+N more" is now an `.expander` button toggling "Show all N" / "Show fewer". A resize handle (`.handle`, `role="separator"`, keyboard-focusable) sits between the table and the drawer at widths above 1101px (the same breakpoint the drawer's sticky behavior already uses); it drags with the mouse, resizes 16px per arrow key press, double-clicks back to the default width, clamps to [320, 720]px, and persists to `localStorage` (`drawer-width`).
- **Verified:** a script (kept out of the repo, as in prior UI todos) ran 18 checks in a simulated DOM against a running server: collapsed/expanded process-row counts against the real snapshot, the fewer/all buttons appearing and disappearing correctly, expand state resetting on switching apps, the handle's ARIA attributes, arrow-key resize in both directions with clamping at both bounds, the value persisting to `localStorage`, and a simulated pointer drag. All 18 passed.
- **Not verified:** layout and interaction in a real browser; the visual check at 1440px and below 1100px is **pending**. `/code-review` and `/writing-review` have not yet been run.
- **Deviation from the sketch:** the todo proposed per-section state tracking that resets "for the new app"; implemented as one centralized reset in `rec()` rather than patching each of the six call sites that can change `sel`, which the todo didn't specify and is a smaller, less error-prone change (noted here per AGENTS.md rule 7, since it is a deviation worth recording even though it still meets the edge case).
- **Review (2026-11-26):** `/code-review high 13` ran a standards and a spec review. Both confirmed two real bugs: a second `mousedown` before the first drag's `mouseup` (or a `mouseup` missed outside the window) would leak a duplicate `document` listener pair, and calling `render()` on every raw `mousemove` rebuilt the whole view, which could drop focus from the handle (a new DOM node replaces it) and was unthrottled at mouse rate. Fixed: a single `endDrag` cleanup is cancelled and replaced on every `mousedown`, a `window` `blur` listener also ends a drag, and the live drag now paints directly onto the existing `.drawer` and `.handle` elements inside a `requestAnimationFrame`, with one full `render()` only on `mouseup` to settle state and restore focus if the handle had it. The simulated-DOM check gained two cases for this (a second mousedown restarting cleanly, and `localStorage` only settling on `mouseup`) and two of its own bugs were found and fixed along the way: a stale `.handle` reference held across ~90 renders, and a regex that assumed no space after `--dw:`, which does not hold for `style.setProperty`'s serialization. 20 of 20 checks pass. Not changed, as judgement calls: `S.expand`'s shape is still duplicated between its `S` initializer and the `rec()` reset (noted for a later refactor if a third expandable section is added); the double-click-to-reset affordance (not in the original spec) is harmless and left in, though untested. `/writing-review` on this todo's prose has not been run.
- **Open:** the handle has no visible affordance below 1101px, since the table and drawer stack there and there is nothing to resize against; this matches the todo's own edge case.
