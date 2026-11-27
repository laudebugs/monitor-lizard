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

- A results menu under the box with groups (Apps, Ports, Hosts, Persistence, Guide), at most 5 per group, each row showing the page it opens.
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

- Typing `3000` offers the Listeners entry for port 3000; Enter opens `#listeners?q=3000` (`writeHash` writes `q`; `port=` is only a read-only shortcut, so this differs from the first draft of this line, deliberately).
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

Built in `web/search.js` (a pure `global-search` block and a `global-search-view` block). Tests: `tests/web/global-search.test.mjs` (matching) and `tests/web/global-search-view.test.mjs` (keys, focus and the menu against a stand-in DOM).

**Pages and addresses.** Apps open `#apps?sel=<key>`. A port opens `#listeners?q=<port>`. A host opens `#network?q=<address>`, or `#history?q=<address>` when it is no longer connected. A startup item opens `#persist?q=<label>`. A Guide entry opens `#guide?q=<typed query>&entry=<id>` (`tab=glossary` for a term), so the Guide lists what the menu offered. A lone surrogate in a key becomes U+FFFD, so the app's "no longer running" notice shows and the selection is not dropped without a word.

**Decisions to confirm.** The todo left these open; I chose:
- A port opens `?q=` and not `?port=`, as above.
- On a page with no filter (Overview, Privilege, Performance) typing no longer jumps to Apps. This closes the Performance gap from S2a. Enter with no result chosen opens Apps with the text.
- On a page that filters in place, the filter still applies while you type and the menu opens beside it. Enter with no result chosen keeps the filter.
- Hosts match a single word only (an address has no spaces); ports match a single number.
- One name for the startup page: Persistence, in the group heading, the rows and the announcements.
- A second Escape, with the menu already closed, empties the box and the page filter. A dialog or the open sidebar takes the Escape first.

**Other choices.**
- Matching ignores case, width variants and invisible format characters (zero width, bidi marks) on both sides, so a hostile name cannot hide. Result text goes through `visible()` into text nodes, and the query read to a screen reader is escaped the same way.
- Apps are the ones the Apps page lists (with network activity). Hosts also include those only in the history.
- The group with the better match (exact, prefix, word start, contains) comes first; equal groups keep the order Apps, Ports, Hosts, Persistence, Guide. Candidates are kept per match quality (200 each) before the five are chosen, so many weak matches never push out an exact one.
- A result on another page is a new history entry; on the page you are on it replaces the entry, as N1 does for filters.
- `/` works on every page and selects the text. It types a slash inside a field. It does nothing while a dialog, the open sidebar, the copy box or a dropdown has the keyboard, or while an input method is composing.
- The arrow, Enter and Escape keys are left alone during composition and when a modifier key is held.
- The box shows the page's filter: it is emptied on pages with no filter and refilled from the page filter when you come back, unless it has focus.
- The menu keeps the highlighted row (by address) and the focus through a poll, redraws when a row's text changes, and announces only when the summary changes.

**Verified.** `make check`, `make test`, `make check-docs`. The orchestrator ran the build in Chrome on a scratch server with a scratch HOME and real machine data: `/` focuses, `sys` shows grouped results (Apps, Guide), the menu and the active option survived 8 seconds of polls, Enter opened Apps with the app selected, and typing on Overview did not leave the page.

**Not verified.** In my own session the harness classifier denied `monitor-lizard serve` with a scratch HOME, so I ran no browser. Not checked by anyone: forced colors, the narrow layout, a screen reader, the light theme and the Storage page's folder search next to the menu.
