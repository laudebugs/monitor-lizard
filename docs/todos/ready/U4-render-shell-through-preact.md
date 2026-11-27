# U4: Render the shell through Preact behind an opt-in

| Field | Value |
|---|---|
| ID | `U4` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D4, D5, D6 |
| Depends on | U1, U3 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `feat/u4-preact-renderer` |
| PR title | `feat(web): render the page through Preact when opted in` |
| Size | L (over 400: split into U4a and U4b if the diff passes 400 lines) |
| PR | — |

## Goal

Add the Preact renderer next to the old one. With `localStorage` key `monitor-lizard-ui` set to `preact`, `render()` builds the page in vnode mode and commits it with Preact into `#content` and `#layer`; without it nothing changes. This is the hard step: it settles the `h()` mode, container ownership, module loading and the post-commit table pass.

## Context

- Plan decisions D2, D4, D5, D6 and the spike evidence (module order, non-empty container, `decorateTables` hook).
- `web/shell.js` `render()` and `show()`; `web/core.js` `h`, `rec`, `quiet`; `web/table.js` `decorateTables`.
- `tests/web/page.mjs` and `tests/web/split.test.mjs` read `<script src>` tags and the load order; a `type="module"` tag must be recognised.

## Scope

**In scope:**

- `h` stays a `let`. `withVnodes(fn)` sets it to a vnode builder for the duration of `fn` and restores it in `finally`: `(tag, attrs, ...kids) => preactH(tag, attrs, ...kids)`. Preact's own `h` already ignores `null` and `false` and flattens arrays.
- `shell.js` becomes `<script type="module" src="/js/shell.js">`; it imports `{ h as ph, render as prender, Fragment } from 'preact'`. It is the last script in the shell, so every classic global exists. Update the tag regex in `page.mjs`, the `SOURCES` order test, and `split.test.mjs`.
- In `render()`: when the opt-in is on, `page = withVnodes(() => pageFn())`; commit `prender(ph(Fragment, null, ...page.nodes), $('content'))`, and the layer the same way into `#layer`; then `decorateTables()` (needed after every commit). The crumb, nav and theme rows move in the same way.
- Container hand-over: switching the opt-in or a page that is not yet vnode-safe first calls `prender(null, container)` then `container.replaceChildren()`. A page that throws in vnode mode falls back to the old path for that render and logs once.
- The opt-in is read once at load; there is no visible switch, and `README` mentions the key only in the plan.

**Out of scope** (name the todo that covers it):

- Removing the old path, `quiet()` or the focus code (U5 to U7). Page-by-page fixes (U6*).

## Target files

| File | Change |
|---|---|
| `web/shell.js` | module; mode switch; commit through Preact |
| `web/core.js` | `withVnodes`, mode variable |
| `web/index.html` | `type="module"` on the shell tag |
| `src/web.rs` | no change expected (same paths); update the order test if it keys on tag text |
| `tests/web/page.mjs`, `tests/web/split.test.mjs` | recognise the module tag, load order, token location |

## Steps

- [ ] Add `withVnodes` and test that `h` is restored after a throw.
- [ ] Make `shell.js` a module; run `make test` (the split test must still pass or be updated for the right reason).
- [ ] Commit through Preact for `#content` only; check the Overview page in a browser with the opt-in.
- [ ] Add `#layer`, crumb, nav, theme rows.
- [ ] For every view, diff the DOM from both paths on live data in the browser (normalise `style` through `cssText`) and record the result.
- [ ] Switch the opt-in on and off repeatedly while a menu is open; check nothing duplicates or disappears.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Opt-in key absent, `null`, `old` | old renderer, byte for byte as today |
| Opt-in on and a page builder throws | that render falls back to the old path, one console message, no blank page |
| Switch from old to Preact path mid-session (reload) | `#content` has exactly one set of children |
| `render(null, container)` then `replaceChildren` | no leftover nodes, no detached handlers firing |
| A poll arrives while a dropdown is open | unchanged behaviour (`quiet()` still guards) |
| `localStorage` blocked | old path, no error |
| Tauri window | not verified by this todo; check in the menu-bar shell and record |
| Table with a hidden column after a commit | columns still hidden; Copy layer realigned |

## Acceptance criteria

- Default behaviour is unchanged (no opt-in).
- With the opt-in every view renders the same DOM as the old path (apart from `style` text), with the 5 `svg` bodies present.
- A focused element in the page stays focused across a poll on the Preact path.
- No console errors on any view on either path.

## Tests

Node: `withVnodes` restore, mode selection, container hand-over with the U2 stand-in (about 10 cases). Browser: every view on both paths, recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` (`web/*.js` row: `shell.js` is the module; load order note), `AGENTS.md` web line if its wording changes.

## Risks and open questions

- Module scripts are deferred; code in `shell.js` that other classic scripts call at load (none today) would break. Check with the split test.
- Webview behaviour of the import map is not verified.
- If the diff passes 400 lines, split: U4a (mode, module, `#content`), U4b (layer, crumb, nav).

## Rollback and how the page keeps working

- **Rollback:** Remove the opt-in key (the default path never changed) or revert the PR (restores the classic `shell.js`).
- **Why the page works at every step:** The default is the old path, and the Preact path is only taken by explicit opt-in with a per-render fallback to the old path.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
