# U5: Dialogs and focus as components

| Field | Value |
|---|---|
| ID | `U5` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D7 |
| Depends on | U4 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u5-dialog-components` |
| PR title | `refactor(web): dialogs as components; focus and inert follow the dialog` |
| Size | M (200-400) |
| PR | — |

## Goal

Move the Stop dialog, the Move-to-Trash dialog, the toast and the copy box onto the Preact renderer as components, and delete their focus and `inert` plumbing there. If a dialog fails to draw, the page stays usable and the dialog closes.

## Context

- The spike converted the Trash dialog: refs on the overlay (sets `inert` and returns focus on unmount) and on a heading keyed by phase (focuses it on mount) replaced `trashFocusBack`'s `setTimeout`, the focus branch in `trashDispatch`, the `data-autofocus` query and the `shell.inert` assignment. An unrelated `rec()` and a poll kept focus and the dialog node.
- `web/page-storage.js` `storage-trash-view` block (133 lines), `web/shell.js` layer code. The code from S3b (#69) is on `main`; its todo file stays in `ongoing/`.
- The pure state machine in `storage-trash` stays; its three `{ do: 'focus' }` effects become unnecessary and are removed with their tests.

## Scope

**In scope:**

- Components `TrashDialog`, `StopDialog`, `Toast`, `CopyBox` and `Layer()`.
- Focus and `inert` through refs, never as a side effect inside a render function: the overlay ref sets `.shell.inert` and returns focus to `#st-trash-btn` on unmount; a heading with `key` = phase takes focus on mount. Because Preact calls a ref again when its function identity changes, the ref functions are module-level constants.
- The draw-failure guard: the existing try/catch stays, closes the dialog, announces, and leaves the page usable.
- Delete on the Preact renderer: `trashFocusBack`'s timers, the `data-autofocus` query, the `!trashOpen()` clause in `quiet()`. `trashKeys` (the Tab trap) stays.
- Poll-while-focused tests for the Trash dialog and for the column menu (checked in a browser and recorded; the vnode tests assert the refs and keys exist).

**Out of scope** (name the todo that covers it):

- Menus and dropdowns keep `quiet()` until U7; deleting the old renderer (U7).

## Target files

| File | Change |
|---|---|
| `web/page-storage.js`, `web/shell.js`, `web/core.js` | components, refs, deletions on the Preact renderer |
| `tests/web/storage-trash-view.test.mjs`, `tests/web/storage-trash.test.mjs` | vnode stand-in cases; focus effects removed |

## Steps

- [ ] Port each dialog with its text and `aria-*` unchanged.
- [ ] Browser, switch on: open; Tab around (wraps both ends); Escape closes one layer; an unrelated `rec()` and a poll do not move focus or rebuild the dialog; close returns focus to the button; a phase change focuses the heading.
- [ ] Force a draw error: the dialog closes, it is announced, the page is not inert.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Hostile folder name (bidi override, newline, zero-width) | shown escaped through `visible()` |
| Poll during `moving` | the dialog keeps its state and focus |
| Escape with Trash over the Stop dialog | closes Trash only |
| Dialog throws while drawing | closed, announced, page not inert |
| Rescan changes the selection while open | the state machine closes it; focus returns once |
| Shift+Tab from the heading | goes to the last button |
| A ref called twice (function identity changes) | not possible: refs are module-level constants (test asserts identity) |

## Acceptance criteria

- All dialog behaviour as before; on the Preact renderer no timer or manual focus restore remains for dialogs.
- Maintainability: record lines and workaround counts for the `storage-trash-view` block before and after (the spike measured about 14 lines of 133 removable), and that the dialog markup tests need no fake DOM.

## Tests

Existing storage-trash tests stay green on the vnode stand-in; add the draw-error case; browser checks recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/storage-trash*.test.mjs
```

## Docs to update

`docs/agents/layout.md` (`storage-trash-view` description).

## Risks and open questions

- `inert` sits on an element outside the container; it must clear when the dialog closes and when the key is removed.

## Rollback and how the dashboard keeps working

- **Rollback:** Remove the key or revert the PR; the old dialog code is untouched until U7.
- **Why the dashboard works at every step:** The old renderer keeps its dialog code; the components run only with the key set.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
