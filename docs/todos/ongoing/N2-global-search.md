# N2: Global search

| Field | Value |
|---|---|
| ID | `N2` |
| Status | `ongoing` |
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

- [x] Write `searchAll` as a pure function with tests first.
- [x] Add the menu with `role="listbox"` and `aria-activedescendant`.
- [ ] Check with keyboard only and with a screen reader label (pending: see the Implementation Record).

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

Built in `web/search.js`; the real-browser check is pending (see Not verified).

**Pages and addresses.** Apps open `#apps?sel=<key>`. A port opens `#listeners?q=<port>`: `writeHash` writes `q`, and the `port=` form is a read-only shortcut, so the acceptance line's `#listeners?port=3000` is met in effect, not in text. A host opens `#network?q=<address>`, or `#history?q=<address>` when it is no longer connected. A startup item opens `#persist?q=<label>`. A Guide entry opens `#guide?q=<typed query>&entry=<id>` (`tab=glossary` for a term), so the Guide lists what the menu offered.

**Decisions the todo left open.**
- Typing on a page with no filter (Overview, Privilege, Performance) no longer jumps to Apps. The menu shows, and Enter with no result chosen opens Apps with the text. This closes the Performance gap recorded in S2a.
- On a page that filters in place, the filter still applies while you type, and the menu shows beside it. Enter with no result chosen keeps the filter.
- Matching ignores case, width variants and invisible format characters (zero width, bidi marks) on both sides, so a hostile name cannot hide from a search for what it looks like. Result text goes through `visible()` and into text nodes.
- Ports match a number by prefix, one row per port. Hosts match an address by prefix, with or without brackets, and only a single word. Apps are the ones the Apps page lists (with network activity).
- Groups with the better match come first (exact, then prefix, then word start, then contains); equal groups keep the order Apps, Ports, Hosts, Startup items, Guide. A query like `snapshots` has no app, port or host match, so the Guide leads.
- Candidates are kept per match quality (200 each) before the five are chosen, so many weak matches never push out an exact one.
- A result on another page is a new history entry; a result on the page you are on replaces the entry, as N1 does for filters.
- `/` works on every page and selects the text in the box. It is ignored in a field, in a dialog and during composition. The keys are ignored while an input method is composing.

**Not verified.** No real-browser run: the harness refused to start a server with a scratch HOME, so the menu's look, focus, themes, forced colors and screen reader output are pending.
