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

Give the Node tests a way to build and inspect component output without a DOM library: a stand-in `h` that returns plain `{ type, props, children }` objects and a walker that reads text, finds elements by attribute and runs handlers. Prove it by porting one dialog test that today needs the `El` stand-in.

## Context

- ADR 0005, "Tests": no new dev dependency. The spike branch has a 29-line prototype (`tests/web/trash-vnode.test.mjs`) to start from; it is throwaway.
- `tests/web/storage-trash-view.test.mjs` has the `El` stand-in (about 40 lines of fake DOM and selector support) and a `world()` helper.
- Plan D8's open question: is the stand-in enough for the dialog tests?

## Scope

**In scope:**

- `tests/web/vnode.mjs`: `h(type, props, ...kids)` flattening arrays and dropping `null` and `false` (the same rule as `nodesOf`), keeping `0`; `text(v)`, `find(v, pred)`, `findAll`, `attr(v, name)`, `click(v)` (calls `onClick`), `toHtml(v)` escaping text and attribute values.
- Port the markup and handler cases of `storage-trash-view.test.mjs`; leave its focus and `inert` cases on the `El` stand-in and list them in the Implementation Record as "needs a browser".

**Out of scope** (name the todo that covers it):

- Any change under `web/`. Real Preact in Node: not done.

## Target files

| File | Change |
|---|---|
| `tests/web/vnode.mjs`, `tests/web/vnode.test.mjs` | (new) stand-in and tests of the stand-in |
| `tests/web/storage-trash-view.test.mjs` | markup and handler cases use the vnode stand-in |

## Steps

- [ ] Write the stand-in with tests first (null, false, nested arrays, `0`, text with `<`, `&` and quotes).
- [ ] Port the cases; confirm each still fails when the product code is broken (flip one condition locally).
- [ ] Record which cases need a browser.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `h('p', null, null, false, ['a', ['b']], 0)` | children `a`, `b`, `0` |
| Text `<script>` in a child or attribute | escaped by `toHtml` |
| `click` on an element without `onClick` | throws a clear error |
| A handler that throws | the error reaches the test |
| `className` | not supported; the app writes `class` |

## Acceptance criteria

- Tests pass; the ported file keeps its case count or explains each removed case; no dependency added.
- Maintainability: the ported test file has fewer lines of fake DOM than before (record the numbers).

## Tests

About 15 cases for the stand-in plus the ported cases.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

`docs/agents/layout.md` `tests/web/` row (one sentence).

## Risks and open questions

- Plain objects do not behave like real Preact diffing; the file header says so and points at the browser checks.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** Test-only; no product file changes.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
