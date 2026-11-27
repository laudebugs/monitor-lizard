# U5: Trash and Stop dialogs and focus as components

| Field | Value |
|---|---|
| ID | `U5` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D7 |
| Depends on | U4, S3b |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u5-dialog-components` |
| PR title | `refactor(web): dialogs as components and focus left to the renderer` |
| Size | M (200-400) |
| PR | — |

## Goal

Move the Stop dialog, the Move-to-Trash dialog, the toast and the copy box onto the Preact path as components whose open state is `S`, and delete their focus workarounds on that path. The dialog freeze class of bugs is addressed here: a dialog that cannot draw is an error boundary, not an inert page.

## Context

- `web/page-storage.js` `storage-trash-view` block (`trashLayer`, `trashDialog`, `trashSync`, `trashKeys`, `trashFocusBack`) and `web/shell.js` layer code.
- The state machine in `storage-trash` is pure and stays as is.
- Plan D7; spike (b): focus survives a re-render on the Preact path.
- S3b is `ongoing`: wait for it to land.

## Scope

**In scope:**

- `TrashDialog`, `StopDialog`, `Toast`, `CopyBox` functions returning `h(...)`; `Layer()` composes them.
- Initial focus: `data-autofocus` stays (the one explicit focus move per dialog open), applied once when the dialog appears, not after every render.
- `inert` on `.shell`: set from `Layer()` through a post-commit step (it is an attribute on a node outside the container).
- The throw guard in `trashLayer` becomes: catch the error where the layer is committed, close the dialog, announce, and leave the page usable.
- On the Preact path, `quiet()` still suppresses redraws under an open menu or dialog until U7 (it is safe to keep: the dialog no longer needs it, but the menu code does).
- Delete from the Preact path: `trashFocusBack`'s `setTimeout`, and the restore after `trashSync`. Return focus to `#st-trash-btn` when the dialog closes (one `ref`/post-commit call).

**Out of scope** (name the todo that covers it):

- Menus and dropdowns (they keep `quiet()`); deleting the old path (U7).

## Target files

| File | Change |
|---|---|
| `web/page-storage.js`, `web/shell.js` | dialog components on the Preact path |
| `tests/web/storage-trash-view.test.mjs` | markup cases on the U2 stand-in; focus cases checked in a browser |

## Steps

- [ ] Port each dialog to a component with its existing text and `aria-*` unchanged.
- [ ] Check in a browser with the opt-in: open, Tab around (wraps at both ends), Escape closes one layer, a poll arrives while open and nothing flickers, close returns focus to the button.
- [ ] Force a draw error and confirm the page stays usable and is announced.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Hostile folder name (bidi override, newline, zero-width) | shown escaped through `visible()`, as today |
| Poll arrives during `moving` | dialog keeps its state and focus |
| Escape with Trash open over the Stop dialog | closes Trash only |
| Dialog throws while drawing | closed, announced, page not inert |
| Rescan changes the selection while open | state machine handles it; focus not stolen |
| Shift+Tab from the heading | goes to the last button |

## Acceptance criteria

- All dialog behaviour as before on both paths; on the Preact path no timers or manual focus restores remain for dialogs.

## Tests

Existing storage-trash and view tests stay green; add the draw-error case; browser checks recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` (`storage-trash-view` description).

## Risks and open questions

- `inert` handling touches an element outside the container; make sure it clears when the dialog closes and when the opt-in is switched.

## Rollback and how the page keeps working

- **Rollback:** Clear the opt-in or revert the PR; the old dialog code is untouched until U7.
- **Why the page works at every step:** The old path keeps its dialog code; the components are used only on the opt-in path.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
