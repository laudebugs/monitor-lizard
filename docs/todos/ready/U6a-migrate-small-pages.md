# U6a: Migrate the small pages

| Field | Value |
|---|---|
| ID | `U6a` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U4 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6a-small-pages` |
| PR title | `refactor(web): run Persistence, Listeners, History and Network on the Preact path` |
| Size | M (200-400) |
| PR | — |

## Goal

Make Persistence (26 lines), Listeners (29), History (56) and Network (41) correct on the Preact path, delete their focus workarounds, and add key attributes to their lists. These are the lowest-risk pages and prove the process for the rest.

## Context

- Plan section 5 order and risks (silent failures, keys).
- Each page returns `pageShell(...)` with tables created by `sortHeader`/`paginate` in `web/table.js`.

## Scope

**In scope:**

- For each page: diff the DOM from both paths on live data; give list rows a stable `key`; remove any `data-req` focus use; add vnode tests for the page's markup helpers.

**Out of scope** (name the todo that covers it):

- Guide, Overview, Apps, Privilege (U6b); Performance (U6c); Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-persistence.js`, `web/page-listeners.js`, `web/page-history.js`, `web/page-network.js` | keys, neutral helpers |
| `tests/web/*.test.mjs` | vnode markup cases |

## Steps

- [ ] For each page, opt in, open it, sort, filter, page, toggle a column, export CSV, copy a row, and keep a row button focused through a poll.
- [ ] Record the DOM diff of both paths per page.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Zero rows | the empty state, not an empty table |
| Sort while a row button is focused | focus stays on a button (the same one if its key is unchanged) |
| A row disappears after a poll | no error, focus falls back to the table |
| 1,000 rows (Network) | page size caps the DOM; update stays under 5 ms in the browser |
| Hidden column then new data | still hidden, Copy layer realigned |

## Acceptance criteria

- Each page matches the old path in the DOM diff and in behaviour; no `data-req` left in these pages.

## Tests

About 6 vnode cases per page; browser checks recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` only if a rule changes.

## Risks and open questions

- History and Network read `S.hist` and `S.snap` that polls replace; keys must come from stable ids, not indexes.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR.
- **Why the page works at every step:** The default is still the old path; this PR only changes how the opt-in path treats these pages.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
