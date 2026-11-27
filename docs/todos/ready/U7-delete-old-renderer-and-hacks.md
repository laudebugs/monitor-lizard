# U7: Make Preact the default and delete the old renderer and its workarounds

| Field | Value |
|---|---|
| ID | `U7` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D7, D11 |
| Depends on | U6d |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u7-delete-old-renderer` |
| PR title | `refactor(web): Preact is the renderer; delete quiet(), hadReq and the Node-building h()` |
| Size | M (200-400) |
| PR | — |

## Goal

Make Preact the default, delete the old renderer and the workarounds it needed, and update the documents. Done only when every page has run on the Preact renderer in a real browser. This is where the maintainability gain is collected and measured.

## Context

- Plan section 5; ADR 0005, "Maintainability evidence" is the checklist of what to delete, with the counts on `main` at `e80bce1`.
- Targets: `quiet()` (16 call sites), `hadReq` and `data-req`, `drawNav`'s `had` restore and `navSig`, `guideAct`, `stFocus`, `trashFocusBack`, `gsig`, the Node-building `h`, `show()`, the switch, the `El` stand-in, the `{ do: 'focus' }` effects.

## Scope

**In scope:**

- Delete the code above; `rec` commits through Preact; the sidebar, theme row and layer are components.
- Replace `quiet()` for menus: a menu's open state lives in `S`, so a poll re-renders and the menu stays open. **Add a test** (vnode stand-in for the markup, a browser check recorded for the behaviour) that a poll while a dropdown and the column menu are open leaves them open, with focus on the same item, before `quiet()` is deleted.
- Keep what is not a hack: `data-autofocus` or its ref for a dialog's first focus, `decorateTables()` after commit, `syncHash`.
- Update `AGENTS.md`, `docs/agents/layout.md`, the plan status; ADR 0005 gets an Amended line (its status is the owner's call).
- Record the numbers: lines of `web/*.js` and counts of `quiet(`, `hadReq`, `data-req`, `data-autofocus`, `.focus()`, `document.activeElement`, `dataset.` cached state, `setTimeout(`, before (the ADR's table) and after.

**Out of scope** (name the todo that covers it):

- Signals, hooks, htm (a later plan, if wanted).

## Target files

| File | Change |
|---|---|
| `web/*.js` | deletions as listed |
| `tests/web/*.test.mjs` | the `El` stand-in removed |
| `AGENTS.md`, `docs/agents/layout.md`, `docs/plans/ui-components.md`, `docs/decisions/0005-...md` | final wording; the numbers |

## Steps

- [ ] `grep` shows no `quiet(`, `hadReq`, `data-req`, `replaceChildren` (outside the one hand-back helper, if kept) in `web/`.
- [ ] Walk every view in a real browser with polls running; Tab through each; open every dialog and menu.
- [ ] Fill the before and after table in the plan.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A poll while a dropdown or the column menu is open | stays open; focus on the same item |
| A poll while a dialog is open | the dialog may update; focus kept |
| First load with a slow network | the HTML shell shows "connecting..." as today |
| `import()` fails at load | the page shows a plain message that the dashboard could not load its UI library (the old renderer no longer exists) |
| Blocked `localStorage` | theme falls back; no error |
| Tauri window | the whole page checked and recorded |

## Acceptance criteria

- The grep is empty; every page, dialog and menu works; the menu test exists and passes.
- Maintainability: the PR shows before and after counts for the workaround list above and for lines of `web/*.js`. The target is that every row the ADR marks "Removed" or "Partly" has its removed part at zero (for `quiet()`, zero call sites), and the rows marked "Stays" are unchanged and named as such. If a row did not shrink, the PR says so.

## Tests

Full web test suite; a complete manual pass recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

`AGENTS.md`, `docs/agents/layout.md`, ADR 0005 (Amended note), plan status Done.

## Risks and open questions

- Menus relied on `quiet()` to avoid closing under a poll; with state-driven rendering this must be re-verified, and a failed `import()` now has no fallback renderer, so decide the message and whether a cached copy is wanted.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR (one squash commit restores the old renderer and the switch).
- **Why the dashboard works at every step:** The old renderer is deleted only after U6a to U6d ran every page on the new one, and the revert restores it in one step.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
