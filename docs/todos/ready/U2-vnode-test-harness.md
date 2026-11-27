# U2: Vnode test stand-in

| Field | Value |
|---|---|
| ID | `U2` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D8 |
| Depends on | None |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `test/u2-vnode-harness` |
| PR title | `test(web): a vnode stand-in so component tests need no DOM library` |
| Size | M (200-400) |
| PR | — |

## Goal

Give the Node tests a way to build and inspect component output without jsdom: a stand-in `h` that returns plain `{ type, props, children }` objects and a walker that renders them to text, finds elements by attribute, and runs handlers. Prove it by porting one existing dialog test.

## Context

- ADR 0005, test options: no new dev dependency.
- `tests/web/storage-trash-view.test.mjs` has a hand-written `El` class and a `world()` helper; `tests/web/page.mjs` assembles the page.
- Plan D8 and the open question: is the stand-in enough for focus assertions?

## Scope

**In scope:**

- `tests/web/vnode.mjs`: `h(type, props, ...kids)` flattening arrays and dropping `null`, `false` (the same rule as `nodesOf`), turning numbers into text; `text(v)`, `find(v, pred)`, `findAll`, `attr(v, name)`, `click(v)` (calls `onClick`), and `toHtml(v)` escaping text and attribute values.
- Port `storage-trash-view.test.mjs` to the stand-in for its markup and handler cases; leave its focus cases on `El` and list them in the Implementation Record as "needs a browser".

**Out of scope** (name the todo that covers it):

- Any change under `web/`. Real Preact in Node (no DOM): not done.

## Target files

| File | Change |
|---|---|
| `tests/web/vnode.mjs` | (new) stand-in and walker |
| `tests/web/vnode.test.mjs` | (new) tests of the stand-in itself |
| `tests/web/storage-trash-view.test.mjs` | markup and handler cases use the stand-in |

## Steps

- [ ] Write the stand-in with tests first (null, false, nested arrays, numbers, strings with `<`, `&`, quotes).
- [ ] Port the dialog markup cases; confirm each still fails when the product code is broken (flip one condition locally).
- [ ] List what cannot be asserted without a browser.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `h('p', null, null, false, ['a', ['b']], 0)` | children are `a`, `b`, `0` (zero is kept) |
| Text `<script>` in a child or attribute | escaped by `toHtml` |
| `click` on an element without `onClick` | throws a clear error |
| A handler that throws | the error reaches the test |
| `class` vs `className` | the stand-in reads `class` only, as the app writes |

## Acceptance criteria

- Tests pass; the ported file keeps its case count or explains each removed case; no dependency added.

## Tests

About 15 cases for the stand-in plus the ported cases.

## Verification

```sh
node --test tests/web/*.test.mjs
make check
make test
```

## Docs to update

`docs/agents/layout.md` `tests/web/` row (one sentence).

## Risks and open questions

- Plain-object vnodes do not behave like real Preact diffing; the file header must say so and point at the browser checks.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR.
- **Why the page works at every step:** Test-only; no product file changes.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
