# N1: URL state and breadcrumbs

| Field | Value |
|---|---|
| ID | `N1` |
| Status | `done` |
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

- [x] Write the parse and write functions as pure functions with tests first.
- [x] Wire `rec()` and the initial load.
- [x] Add the breadcrumb to each page's header.
- [x] Open a selected-item link in a new tab and check it lands on the same item.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-26)

**Built.** The dashboard keeps its state in `location.hash`, as `#<page>?<key>=<value>&...`. The state is the page, chip filter, dropdown filters, search text, sort and selected app. It is restored on load, on reload and on Back or Forward. The key list is documented in the header comment of the `url-state` block in `web/index.html`.

Page and selection changes push a history entry. Filter, sort and search edits replace it, so Back skips them. Each page shows a breadcrumb (`Sys Monitor > Page > selected app`) made of links.

An address is untrusted input. Every page, filter value, dropdown value, sort key, sort column and direction is checked against a fixed list, and lookups use own properties only, so names such as `__proto__` and `constructor` are ignored. Over 2 KB the hash drops the selection, then the filters, then the sort. A value that cannot be encoded is skipped on its own. The write token is never read into or written to the hash.

**Tests.** `tests/web/url-state.test.mjs` has 31 tests run by `node --test`. They cover every edge case in the table above except Back and Forward in a browser. The repo had no JavaScript test setup, and the simulated DOM harness that AGENTS.md mentions is not in the tree. The test therefore lifts the marked block out of the page and runs it with fake `location` and `history` objects. This needs no build step or exports. `make test` runs it after `cargo test`, and CI's macOS job gained a `setup-node` step.

**Found in review and the browser.**

- The page defined a global `function history()` (the History view), which replaced `window.history`. The first version never updated the address. The view is now `historyView()`, and a test fails if a global with that name returns.
- `#apps?sort=apps.__proto__.asc` blanked the page, because the sort column was looked up on `Object.prototype`. Columns are now validated per page, a test compares the list with the `sortHeader` calls, and `sortRows` uses an own-property check.
- Dropdown filters were first left out of the hash. They are now in it, so a copied link shows the same rows.

**Deviations.**

- `host`, `port`, `exposed` and `failed` are read-only shortcuts. The page writes the canonical keys instead: the search text, the Listeners chip and `result`.
- The keys `label`, `signing`, `proto` and `result` are additions to the todo's list, for the dropdown filters.
- Page number and page size are not in the hash.
- The notice for a vanished selection is the existing toast. It shows only when the selection came from the address, not when an app quits during polling.

**Checked in a real browser** (Chrome, 2026-11-26, this branch's build on a spare port):

- Overview, Apps, the Needs review chip, a column sort, a dropdown filter and a selected app gave one history entry for the page and one for the selection. The chip, sort and dropdown added none.
- Back cleared the selection, Back again reached Overview, and Forward twice restored filter, dropdown, sort and selection.
- A fresh document on the same URL showed the same rows and selection at 1000 px and at 400 px wide, in the light theme. At 400 px the breadcrumb fit without overflow.
- The crash link above and its equivalents on Listeners, Persistence, Network, History and Privilege rendered each page with the bad sort ignored.
- A URL naming an app that is not running opened Apps with no selection, the toast and a hash without `sel`.

**Open.**

- Not checked in a browser: Back and Forward through the other pages, cmd-click on a crumb, and the `host`, `port` and `exposed` shortcuts (unit tests only).
- The history entry after a vanished selection is covered by unit tests only, because an iframe shares the tab's history count.
- The Counts and Related links that point at hashes belong to the page redesign todos; only the format exists now.
- `/writing-review` is for the owner's session.
