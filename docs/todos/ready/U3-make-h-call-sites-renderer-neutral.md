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

Remove the contract that `h()` returns a live Node, while the old renderer is still the only one. After this PR every site works whether `h` returns a Node or a vnode, so U4 can switch modes. No visible change.

## Context

- Plan section 2 lists the patterns and counts; the spike showed only these break, and one of them silently.
- `web/core.js` (`h`, `nodesOf`), `web/shell.js`, `web/table.js`, `web/page-performance.js`, `web/page-storage.js`.
- S2a, S2b, S3b and B7 are `ongoing` and touch `page-storage.js` and `page-performance.js`: rebase onto them, or wait for them to land.

## Scope

**In scope:**

- `svgBody(attrs, markup)` helper: in DOM mode creates the `svg` and sets `innerHTML`; in vnode mode returns `h('svg', { ...attrs, dangerouslySetInnerHTML: { __html: markup } })`. Replace the 5 sites (nav icon, sparkline, multi-line chart, stacked chart, capacity chart). Markup is built only from numbers and our own constants, as today.
- `download()` builds its anchor with `document.createElement`, not `h`.
- The Copy layer in `decorateTables()` keeps building real elements with a local `el()` helper (it is DOM post-processing by design, plan D6).
- Keep `nodesOf` as it is, and make `pageShell` ask whether `filters` is empty through one helper that works for Nodes and vnodes, with a test.
- The sparkline width read becomes a parameter with a default, not a `document.querySelector` inside the builder.
- A `grep` check (a Node test over `web/*.js`) that fails on `\.innerHTML\s*=`, `\.append(` and `.appendChild(` outside `core.js` and `table.js`'s named helpers.

**Out of scope** (name the todo that covers it):

- The Preact renderer itself (U4).

## Target files

| File | Change |
|---|---|
| `web/core.js` | `svgBody`, `el` helper, mode variable (`let h`) with DOM mode only |
| `web/shell.js`, `web/page-performance.js`, `web/page-storage.js`, `web/table.js` | the 5 svg sites, the anchor, the layer, the width read |
| `tests/web/*.test.mjs` | the grep guard; `svgBody` in both modes using the U2 stand-in |

## Steps

- [ ] Write the grep guard first; it fails on today's code.
- [ ] Replace each site; after each, load the page in a browser and compare the chart or icon with a screenshot taken before the change.
- [ ] Add `svgBody` tests (DOM mode on the existing `El`-style stand-in, vnode mode on the U2 stand-in).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `svgBody` with empty markup | an empty `svg`, no error |
| Markup containing `</svg>` | not produced by our builders; the test documents that inputs are numbers only |
| Nav icon in the sidebar | identical to today (compare in a browser) |
| `nodesOf([filters])` where `filters` is `[null, false]` | empty, so no filter bar |
| Download of CSV and JSON | file saved with the same name and content |
| Copy buttons after a column is hidden | still aligned to their rows |

## Acceptance criteria

- No `.innerHTML =` outside `svgBody`; the guard test is in `make test`.
- Charts, sparklines, the nav icons, export and row copy look and behave as before, checked in a real browser and recorded.

## Tests

Node: the guard plus about 8 cases. Browser: Performance (all five tabs), Storage capacity chart, sidebar icons, one CSV export, one row copy.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` (the `h` contract in one sentence).

## Risks and open questions

- Rebase cost with the in-flight Storage and Performance todos.
- A site found later that the grep misses: U4 diffs both renderers on real pages to catch it.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR; it only replaces equivalent code.
- **Why the page works at every step:** It stays on the old renderer; every replaced site is behaviour-preserving and checked in a browser.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
