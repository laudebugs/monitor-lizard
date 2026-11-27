# U6a: Migrate the small pages

| Field | Value |
|---|---|
| ID | `U6a` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U4 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6a-small-pages` |
| PR title | `refactor(web): run Persistence, Listeners, History and Network on the Preact renderer` |
| Size | M (200-400) |
| PR | — |

## Goal

Make Persistence, Listeners, History and Network work on the Preact renderer, delete their workarounds, and key their lists. "Work" means: the DOM equals the old renderer's on live data (apart from `style` text), sort, filter, paging, column hiding, CSV export and row copy behave as before, scroll position survives a poll, and a focused row button stays focused.

## Context

- Plan section 5 order; risks: silent failures, keys, foreign mutations on Preact's DOM.
- Tables come from `sortHeader` and `paginate` in `web/table.js`.

## Scope

**In scope:**

- Per page: DOM diff on both renderers; stable `key` on each list row (from the record's id, never the index); delete any `data-req`-style focus code; escape every server or process text through `visible()` and test it with a bidi, newline and zero-width name; markup helper tests on the vnode stand-in.

**Out of scope** (name the todo that covers it):

- Guide, Overview, Apps, Privilege (U6b); Performance (U6c); Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-persistence.js`, `web/page-listeners.js`, `web/page-history.js`, `web/page-network.js` | keys, neutral helpers |
| `tests/web/*.test.mjs` | vnode markup cases |

## Steps

- [ ] Per page, with the key set: open it, sort, filter, page, hide a column, export CSV, copy a row, scroll a long table and let a poll arrive (the scroll position must hold), keep a row button focused through a poll.
- [ ] Record the DOM diff of both renderers per page.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Zero rows | the empty state, not an empty table |
| Sort while a row button is focused | focus stays on a button (the same one if its key is unchanged) |
| A row disappears after a poll | no error; focus falls back to the table |
| 1,000 rows (Network) | the page size caps the DOM |
| Hidden column then new data | still hidden; the Copy layer realigned once |
| A hostile process or host name | escaped by `visible()`; no raw bidi, newline or zero-width character |
| Rows reordered by sort | keys keep each row's own element and focus |

## Acceptance criteria

- Each page matches the old renderer in DOM and behaviour.
- **Maintainability (the reason for this plan):** each page or component moved deletes the workarounds it needed (a `data-req` or `stFocus`-style focus restore, a `quiet()` guard, a dataset-cached state) and the PR description records before and after counts of those workarounds and of lines in the files touched. Logic that a page owns (labels, rows, text) is tested in Node against the vnode stand-in, with no browser needed; only behaviour that needs a real browser (focus, `inert`, scroll) is checked by hand.

## Tests

About 6 vnode cases per page (markup, keys, escaping); browser checks recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

`docs/agents/layout.md` only if a rule changes.

## Risks and open questions

- History and Network read data that polls replace; keys must come from stable ids.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** The default is still the old renderer; this PR only changes how the Preact renderer treats these pages.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
