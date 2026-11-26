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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-26)

**Built.** `web/index.html` keeps the page, chip filter, search text, sort and selected app in `location.hash` as `#<page>?<key>=<value>&...`, and restores them on load and on `hashchange`. The pure functions `parseHash`, `writeHash`, `historyMode`, `hashStateOf`, `patchOf`, `syncHash` and `crumbParts` sit in one block marked `url-state:begin/end`; the key list is documented in that block's comment. `render()` ends by calling `syncHash`, so every path that changes state (`rec()`, polls, the search box) is covered. Page and selection changes push a history entry; filter, sort and search edits replace it. Each page has a breadcrumb (`Sys Monitor > Page > selected app`) made of links. A selected app that no longer exists is cleared with a toast. Hashes over 2 KB drop the selection, then the filters. The write token is never read into or written to the hash.

**Tests.** `tests/web/url-state.test.mjs` (19 tests, `node --test`) covers every edge case in the table above except Back/Forward in a browser. The repo had no JavaScript test setup and no simulated-DOM harness (AGENTS.md mentions one, but none is in the tree), so this is the smallest fit: the test lifts the marked block out of the page and runs it with fake `location` and `history` objects, which keeps the page free of a build step and of exports. `make test` now runs it after `cargo test`; CI's macOS job gained a `setup-node` step for it.

**Deviations.**

- Dropdown filters (`S.fil`, such as the Apps label and signing filters) are not in the hash, because the todo lists a fixed key set. Only the Privilege "failed" result filter is, as `failed=1`. A shared link therefore does not carry those dropdowns.
- `host`, `port` and `exposed` are read-only shortcuts that map onto the search text or the Listeners chip; the page never writes them. `failed` is read and written.
- Row page numbers and page size are not in the hash.
- The toast for a vanished selection is the existing toast, not a new notice element.

**Open.**

- Pending, not run (the browser extension is not connected): layout of the breadcrumb at desktop and phone width, light and dark; Back and Forward through pages and selections; opening a selected-app link in a new tab; reload on a filtered, sorted view; the vanished-selection toast. The todo asks for these by hand in a real browser.
- The Counts and Related links that point at hashes belong to the page redesign todos; only the format exists now.
- `/code-review` and `/writing-review` are for the owner's session.
