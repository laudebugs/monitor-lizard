# U3: Make every h() call site renderer-neutral

| Field | Value |
|---|---|
| ID | `U3` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D3 |
| Depends on | U2 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u3-neutral-h` |
| PR title | `refactor(web): stop treating h() results as DOM nodes` |
| Size | M (200-400) |
| PR | — |

## Goal

Remove the contract that `h()` returns a live Node, while the old renderer is still the only one. Afterwards every site works whether `h` returns a Node or a vnode, so U4 can switch. No visible change. This also removes the thing that blocks refactoring the builder.

## Context

- Plan section 2 lists the patterns; the spike showed which break, and that one breaks silently.
- B7, S2a, S2b and S3b are merged; the code in `page-storage.js` and `page-performance.js` is on `main`. Their todo files stay in `ongoing/` for owner-run checks and are not touched here.

## Scope

**In scope:**

- `svgBody(attrs, markup)` helper: in Node mode creates the `svg` and sets `innerHTML`; in vnode mode returns `h('svg', { ...attrs, dangerouslySetInnerHTML: { __html: markup } })`. Replace the 5 sites. Markup comes only from numbers and our own constants.
- `download()` builds its anchor with `document.createElement`, not `h`.
- The Copy layer in `decorateTables()` builds real elements with a local `el()` helper (DOM post-processing by design, D6).
- `pageShell` asks whether `filters` is empty through one helper that works for Nodes and vnodes.
- The sparkline width read becomes a parameter with a default (no `document.querySelector` inside the builder).
- `h()` no longer uses `instanceof Node` for its own children rule: a helper `isNode(x)` is used and tested for Nodes and vnodes.
- **Guards** (one Node test over `web/*.js`, each failing on today's code until fixed, with an allow-list of named helpers):
  1. `.innerHTML =`, `.append(`, `.appendChild(`, `.replaceChildren(` only in `svgBody`, `download`, the Copy-layer helper and the renderer functions named in the list.
  2. `dangerouslySetInnerHTML` appears only inside `svgBody` (the one sink that can carry markup), and no Preact child is ever built by concatenating text into HTML.
  3. `.style.setProperty`, `.classList.` and `.setAttribute(` on live nodes only in `table.js` post-processing (`setFloor`, `decorateTables`) and the drawer resize, listed by name.
  4. `dataset.grid` and other values cached on DOM nodes: listed; `tracksOf` keeps working when a reused node holds a stale value (test).
  5. `document.querySelector('.shell')` and `.inert` only in the dialog code.
  6. `instanceof Node` nowhere.
  7. Every `.focus()` call (22 today) is on a list of named sites; a new one fails the test until listed, so U5 and U7 can shrink the list.

**Out of scope** (name the todo that covers it):

- The Preact renderer itself (U4). Deleting the focus calls (U5, U6, U7).

## Target files

| File | Change |
|---|---|
| `web/core.js` | `svgBody`, `isNode`, `let h` |
| `web/shell.js`, `web/page-performance.js`, `web/page-storage.js`, `web/table.js` | the 5 svg sites, the anchor, the Copy-layer helper, the width read |
| `tests/web/guards.test.mjs` | (new) the seven guards |
| `tests/web/*.test.mjs` | `svgBody` in both modes (vnode stand-in from U2 and the `El` stand-in) |

## Steps

- [ ] Write the guards first; they fail on today's code.
- [ ] Replace each site; after each, load the page in a browser and compare the chart or icon with a screenshot taken before the change.
- [ ] Add `svgBody` tests in both modes.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `svgBody` with empty markup | an empty `svg`, no error |
| Markup containing `</svg>` | the builders never produce it; the test documents that inputs are numbers only |
| Nav icon in the sidebar | identical to today (browser) |
| `filters` is `[null, false]` | no filter bar |
| `h('p', null, 'a')` and a vnode child of the same shape | both accepted by `isNode`/kids rule |
| CSV and JSON download | same file name and content |
| Hidden column then new rows | Copy buttons still aligned |
| `dataset.grid` stale on a reused node | `tracksOf` recomputes from the current `style` |

## Acceptance criteria

- No `.innerHTML =` outside `svgBody`; `dangerouslySetInnerHTML` only inside it; all guards in `make test`.
- Charts, sparklines, the nav icons, export and row copy look and behave as before, checked in a real browser and recorded.
- Maintainability: the PR records how many sites still depend on the Node contract (target: 0).

## Tests

Node: the guards plus about 8 cases. Browser: Performance (five tabs), the Storage capacity chart, sidebar icons, one CSV export, one row copy.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/guards.test.mjs
```

## Docs to update

`docs/agents/layout.md` (the `h` contract in one sentence).

## Risks and open questions

- A site the guards miss is found later: U4 diffs both renderers on real pages to catch it.
- A reviewer should read `svgBody` closely: it is the one place that sets markup.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR; it only replaces equivalent code.
- **Why the dashboard works at every step:** It stays on the old renderer; every replaced site keeps its behaviour and is checked in a browser.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
