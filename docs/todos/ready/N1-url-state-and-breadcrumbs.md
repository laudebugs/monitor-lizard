# N1: URL state and breadcrumbs

| Field | Value |
|---|---|
| ID | `N1` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D11, D12 |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/n1-url-state-and-breadcrumbs` |
| PR title | `feat(web): keep page, filters, sort and selection in the URL hash` |
| Size | M (200-400) |
| PR | — |

## Goal

Make every view addressable: page, filters, search text, sort and selected item live in `location.hash`, restore on reload and on Back, and show as a breadcrumb above each page.

## Context

- `web/index.html` keeps state in the object `S` (`view`, `sel`, `filter`, `lfilter`, `pfilter`, `nfilter`, `q`, `sorts`, `fil`) and updates it through `rec()`. Nothing is written to the URL, so a reload returns to Overview.
- The design boards F, G and H show the breadcrumb and the pre-filtered links (`#network?host=203.0.113.44`).

## Scope

**In scope:**

- A hash format `#<page>?<key>=<value>&...` using short, stable keys (`filter`, `sel`, `q`, `sort`, `host`, `port`, `exposed`, `failed`).
- `rec()` writes the hash with `history.replaceState` for filter and sort changes and `pushState` for page changes and selections, so Back steps through pages and selections.
- On load and on `hashchange`, parse the hash into `S`; unknown keys and bad values are ignored.
- Breadcrumb component: `Sys Monitor › Page › selected item`, each part a link.
- Counts and Related links elsewhere can link to a hash.

**Out of scope** (name the todo that covers it):

- The global search box: N2. Restyling: T1a and T1b.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `parseHash`, `writeHash`, `crumb()`, `hashchange` listener, `rec()` |

## Steps

- [ ] Write the parse and write functions as pure functions with tests first.
- [ ] Wire `rec()` and the initial load.
- [ ] Add the breadcrumb to each page's header.
- [ ] Open a selected-item link in a new tab and check it lands on the same item.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `#nonsense` or an unknown page | Overview |
| Selected item no longer exists | Page opens with no selection and a one-line notice |
| Value with `&`, `=` or `#` | Encoded and decoded without loss |
| Hash very long (over 2 KB) | Selection dropped first, then filters; never throws |
| Back after a filter change | Does not add a history entry |
| The per-run write token | Never placed in the hash |

## Acceptance criteria

- Copying the address of a filtered, sorted view into a new tab shows the same rows.
- Reload keeps the page, filters, sort and selection.

## Tests

Logic tests in the simulated DOM harness, 10 minimum, including each edge case. Check Back and Forward by hand in a real browser and record it.

## Verification

```sh
make check
make test
cargo run --release -- serve   # open a filtered view, reload, press Back
```

## Docs to update

Plan status; `docs/agents/` page notes if present.

## Risks and open questions

- Hash keys become a public format once shared; keep the list small and documented in the file's header comment.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
