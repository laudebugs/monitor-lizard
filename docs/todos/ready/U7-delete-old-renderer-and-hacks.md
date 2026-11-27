# U7: Make Preact the default and delete the old renderer and its hacks

| Field | Value |
|---|---|
| ID | `U7` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D7, D11 |
| Depends on | U6d |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u7-delete-old-renderer` |
| PR title | `refactor(web): Preact is the renderer; delete quiet(), hadReq and the DOM-mode h()` |
| Size | M (200-400) |
| PR | — |

## Goal

Flip the default to Preact, delete the old path and the hacks it needed, and update the documents. Done only when every page has run on the Preact path in a real browser.

## Context

- Plan section 5; ADR 0005 "documents that change when this is accepted".
- Targets: `quiet()` (16 call sites become `rec()` or a bare state merge plus the commit), the `hadReq` lookup and `data-req`, `drawNav`'s `had` restore and `navSig`, `guideAct`, `stFocus`, `trashFocusBack`, `gsig`, the DOM-mode `h`, `show()`, the opt-in key, the `El` stand-in.

## Scope

**In scope:**

- Delete the code above; `rec` commits through Preact; `h` returns vnodes always; the sidebar, theme row and layer are components.
- Keep what is not a hack: `data-autofocus` for a dialog's first focus, `decorateTables()` after commit, `syncHash`.
- Update `AGENTS.md`, `docs/agents/layout.md`, ADR 0005 (status is the owner's call), plan status.

**Out of scope** (name the todo that covers it):

- Signals, hooks, htm (a later plan if wanted).

## Target files

| File | Change |
|---|---|
| `web/*.js` | deletions as listed |
| `tests/web/*.test.mjs` | `El` stand-in removed; vnode stand-in only |
| `AGENTS.md`, `docs/agents/layout.md`, `docs/plans/ui-components.md` | final wording |

## Steps

- [ ] `grep` shows no `quiet(`, `hadReq`, `data-req`, `replaceChildren` in `web/`.
- [ ] Walk every view in a real browser with polls running; Tab through each; open every dialog.
- [ ] Record before and after bytes of `web/*.js`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A poll while a menu is open | menu stays open (state-driven, not a skipped render) |
| A poll while a dialog is open | dialog content may update; focus kept |
| First load with slow network | the shell shows "connecting..." as today |
| Blocked `localStorage` | theme falls back; no error |
| Tauri window | check the menu-bar shell and record |

## Acceptance criteria

- The grep above is empty; every page and dialog works; web files are smaller than before this series.

## Tests

Full web test suite; a complete manual pass recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`AGENTS.md`, `docs/agents/layout.md`, ADR 0005 Amended note, plan status Done.

## Risks and open questions

- Menus (`S.menu`) relied on `quiet()` to avoid closing under a poll; with state-driven rendering this must be re-verified.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR (one squash commit restores the old path and the opt-in).
- **Why the page works at every step:** The old path is deleted only after U6a to U6d ran every page on the new one, and the revert restores it in one step.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
