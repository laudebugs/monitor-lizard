# U4: Render the page through Preact behind a switch

| Field | Value |
|---|---|
| ID | `U4` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D4, D5, D6 |
| Depends on | U1, U3 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `feat/u4-preact-renderer` |
| PR title | `feat(web): render the page through Preact when the switch is on` |
| Size | L (over 400: split into U4a and U4b if the diff passes 400 lines) |
| PR | — |

## Goal

Add the Preact renderer beside the old one. When `localStorage` key `monitor-lizard-ui` is `preact` (set in the browser's developer tools; there is no visible control), `render()` builds the page in vnode mode and commits it with Preact. Without the key nothing changes. All scripts stay classic.

## Context

- Plan decisions D2, D4, D5, D6; the spike round two (branch `spike/preact-h-compat`) shows the shape: a `build()` mode switch in `core.js`, a `commit()` helper and one `import()` in `shell.js`.
- `web/shell.js` `render()` and `show()`; `web/core.js` `h`, `rec`; `web/table.js` `decorateTables`.
- Why not a module script: module top-level names are not globals, and `render`, `pageShell`, `show` and `timeCell` are called from other scripts (22, 18, 18 and 11 references).

## Scope

**In scope:**

- `withVnodes(fn)` (spike: `build`) sets `h` to a vnode builder (`(tag, attrs, ...kids) => preact.h(tag, attrs || null, ...kids)`) while `fn` runs and restores it in `finally`.
- `shell.js` stays a classic script. At load, when the switch is on, it runs `import('/vendor/preact.mjs').then((m) => { window.preact = m; render(); }, warn)`. Until then and when the switch is off, `h()` builds Nodes, so the first paint is unchanged.
- `commit(container, nodes)`: if the container is not Preact-owned, call `replaceChildren()` once and mark it owned; then `preact.render(h(Fragment, null, ...nodes), container)`. Handing back (switch removed, or a page that throws in vnode mode) calls `preact.render(null, container)` and then `replaceChildren()` — never `replaceChildren()` alone, which leaves Preact's internal state stale and makes the next render draw nothing.
- `#crumb`, `#content`, `#layer` and the notif and theme rows commit through Preact. `decorateTables()` runs after every commit.
- A page that throws while built in vnode mode falls back to the old renderer for that render and logs once.
- Update the tag-order tests only if they need it: no new script tag is added.

**Out of scope** (name the todo that covers it):

- Removing the old renderer, `quiet()` or the focus code (U5 to U7). Page-by-page fixes (U6*).

## Target files

| File | Change |
|---|---|
| `web/shell.js`, `web/core.js` | the switch, `withVnodes`, `commit`, the `import()` |
| `tests/web/*.test.mjs` | mode selection, hand-back, `withVnodes` restore (vnode stand-in) |

## Steps

- [ ] Add `withVnodes` and test that `h` is restored after a throw.
- [ ] Commit `#content` only first; check Overview with the switch on.
- [ ] Add `#crumb`, `#layer`, the rows; then diff the DOM of both renderers for every view on live data (normalise `style` through `cssText`) and record it.
- [ ] Add and remove the key repeatedly with a menu open; nothing duplicates or disappears.
- [ ] Check the Copy layer after a commit that changes rows, and the column menu while a poll arrives.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Key absent, `null` or `old` | the old renderer, as today |
| Key `preact` and `import()` fails (offline file missing, blocked) | one console warning; the old renderer keeps running |
| Key `preact` and a page builder throws | that render falls back; no blank page |
| Reload with the key set | `#content` has exactly one set of children |
| `preact.render(null, c)` then `replaceChildren` | no leftover nodes or live handlers |
| A poll arrives while a dropdown or the column menu is open | unchanged (`quiet()` still guards) |
| `localStorage` blocked | the old renderer, no error |
| Table with a hidden column after a commit that changes rows | columns still hidden; Copy layer realigned and not duplicated |
| Tauri window | `import()` result recorded (U1 proved the file loads); the whole page checked there |

## Acceptance criteria

- Default behaviour is unchanged.
- With the key set every view renders the same DOM as the old renderer (apart from `style` text), with the 5 `svg` bodies present.
- A focused control stays focused across a poll on the Preact renderer.
- No console errors on any view with the key on or off.
- Maintainability: the PR records the line counts of `shell.js` and `core.js` before and after and lists which workarounds the Preact renderer already makes unnecessary but which stay until U7.

## Tests

Node: `withVnodes`, mode selection and hand-back (about 10 cases). Browser: every view, both renderers, recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

`docs/agents/layout.md` (`web/*.js` row: `shell.js` holds the one `import()`).

## Risks and open questions

- The diff may pass 400 lines: split into U4a (mode, `#content`) and U4b (layer, crumb, rows).
- The `import()` is async, so the first Preact commit happens after the first paint; a page flash is possible and must be checked.

## Rollback and how the dashboard keeps working

- **Rollback:** Remove the key in developer tools (the default never changed), or revert the PR: the revert deletes the `import()`, `window.preact` and `commit`, and the file in `web/vendor/` stays served and unused.
- **Why the dashboard works at every step:** The default is the old renderer; the Preact renderer starts only with the key, and each failure falls back to the old one.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
