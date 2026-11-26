# N2: Global search

| Field | Value |
|---|---|
| ID | `N2` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D13 |
| Depends on | N1, G1 |
| ADRs | None |
| Branch | `feat/n2-global-search` |
| PR title | `feat(web): one search box that finds items on every page` |
| Size | M (200-400) |
| PR | — |

## Goal

Press `/` anywhere, type, and jump to an app, port, remote host, startup item or Guide entry. Today the search box only filters the page you are on (`FILTERABLE`).

## Context

- `S.q` and `FILTERABLE` in `web/index.html` filter the current table only.
- G1 serves the Guide table at `/api/guide`; the other data is already in `S.snap`, `S.seen`, `S.pers` and `S.sudo`.
- Design boards F, G and H show the box with a `/` hint.

## Scope

**In scope:**

- A results menu under the box with groups (Apps, Ports, Hosts, Startup items, Guide), at most 5 per group, each row showing the page it opens.
- Matching: every word must appear in the label, command or address; apps by label and key, ports by number, hosts by IP prefix.
- Keyboard: `/` focuses (ignored while typing in a field), arrow keys move, Enter opens the result through an N1 hash, Esc closes.
- Enter on text with no selection opens the current page's filter as before.

**Out of scope** (name the todo that covers it):

- Searching file names or storage items: not planned. Fuzzy ranking: later.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `searchAll(q)`, results menu, key handling |

## Steps

- [ ] Write `searchAll` as a pure function with tests first.
- [ ] Add the menu with `role="listbox"` and `aria-activedescendant`.
- [ ] Check with keyboard only and with a screen reader label.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Empty text | Menu closed |
| No match | "Nothing matches" row, no error |
| 1,000 apps | Search stays under 50 ms (cap at 200 candidates per group before ranking) |
| `/` pressed inside an input | Types a slash |
| IPv6 address | Matches on any prefix without the brackets |
| Result for an item that disappears | Opens the page with a notice (N1 behavior) |

## Acceptance criteria

- Typing `3000` offers the Listeners entry for port 3000; Enter opens `#listeners?port=3000`.
- Typing `snapshots` offers the Guide entries first.

## Tests

Logic tests, 10 minimum. Check the menu by hand in a real browser and record it.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; README keyboard shortcuts if listed.

## Risks and open questions

- Search text is never sent anywhere; it matches in the browser against data already loaded.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
