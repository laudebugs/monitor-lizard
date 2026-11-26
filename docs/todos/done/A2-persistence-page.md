# A2: Persistence page

| Field | Value |
|---|---|
| ID | `A2` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a2-persistence-page` |
| PR title | `feat(web): add the persistence page and new-item attention card` |
| Size | M (200-400) |
| PR | — |

## Goal

Replace the Persistence placeholder with a page listing every startup item, flag new and unsigned ones, and surface new items on the Overview.

## Context

- [A1](../done/A1-backend-persistence-history-api.md) provides `/api/persistence`.
- The Apps page in `web/index.html` is the pattern: chips, search, sticky header, pagination, drawer-style detail.

## Scope

**In scope:**

- Nav item "Persistence" with a badge of new items.
- Table: label, kind, scope, program, trust, added. Chips: All, New, User, System.
- Attention card on Overview for new items (high if the program is unsigned, else medium).

**Out of scope** (name the todo that covers it):

- Disabling or removing a startup item: a later todo, needs its own safety review.
- Login items: not covered.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove the `persist` entry from `SOON`; add `persistence()` view, loader (every 60 s via `quiet`), `cards()` rule, nav badge |

## Steps

- [x] Add `S.pers`, `loadPers` and the view function using `paginate(list, "pers")`.
- [x] Render `label` and `program` with `textContent` only (h() already does).
- [x] Add the card: "New startup item: <label>" with a Details button that opens the Persistence view.
- [x] Make search (`S.q`) filter label and program on this page.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No items | Empty-state row, no pager |
| Item with `signing: null` (program missing on disk) | Trust column shows "-"; not counted as unsigned |
| `signing.signed === false` | Trust shows UNSIGNED with the bad tag; card severity high |
| Very long program path | Ellipsis with `title` showing the full path |
| Label containing `<script>` or HTML | Shown as literal text |

## Acceptance criteria

- Persistence page lists items from `/api/persistence` with correct counts.
- A new item (simulate by deleting its row from the `persistence` table and restarting) appears as "New" with a badge and an Overview card.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

A jsdom smoke test (kept out of the repo) that renders the page against a running server, opens the view, applies each chip and checks row counts, and asserts no script errors. The layout check needs a real browser.

## Verification

```sh
make check\nmake test\ncargo run --release -- serve
```

## Docs to update

- `docs/agents/layout.md` if files change
- Plan section 2

## Risks and open questions

- No real browser may be available in the agent environment; record layout checks as pending then.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no user-facing prose added beyond existing UI copy patterns; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `persistence()` view in `web/index.html` (chips All/New/User/System, a sortable table with Label/Kind/Scope/Program/Trust/Added, search on label and program, shared `paginate`/`sortRows`/`sortHeader` helpers), `S.pers`/`S.pfilter` state, `loadPers()` polling `/api/persistence` every 60 s, a nav badge for `is_new` items, and an Overview attention card (high severity if unsigned, else medium) with a Details button that opens the Persistence view. `psig(i)` reuses the existing `sigKind(g)` classification rather than re-deriving it (a review fix, see below).
- **Real run on this Mac:** against the real `/api/persistence` endpoint, 37 real startup items (Adobe, Cisco Secure Client, Cloudflare WARP and others), correctly split by Kind/Scope, with one genuine `UNSIGNED` item found and tagged (`com.cisco.secureclient.thousandeyes.uninstall`).
- **New-item scenario:** this Mac's own history has no new items right now (steady state), so the todo's suggested verification -- "simulate by deleting its row from the persistence table and restarting" -- was done against an isolated fake-`HOME` copy of the data directory (never the real `~/.sys-monitor/`), so as not to perturb the owner's actual history: populated a fresh history.db, backdated the remaining rows past the learning period, deleted one row, restarted. The deleted item correctly reappeared with `is_new: true`, the nav badge showed 1, and the Overview card appeared at the correct severity.
- **Real-browser verification:** no Claude-in-Chrome extension was connected this session. Headless Chrome screenshots confirmed the page visually at 1440 px and 1000 px (below the todo's 1100 px threshold), including the empty-state row (User chip, 0 matches) with no pager and a working Clear filters button. Interaction (not just static rendering) was also verified: headless Chrome was driven directly over the Chrome DevTools Protocol (a local WebSocket, no extension needed) to click the Persistence nav item and each chip, confirming rendered row counts match each chip's own printed count (capped at the page size), that search filters in place without jumping to Apps, and that no raw `<script>` tag reaches the table markup. This script is kept in the scratchpad, not committed, per the todo's Tests section.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Spec review found no gaps. Standards review found two real issues, both fixed: `clearFilters()` (shared by every page's empty-state "Clear filters" button) reset `filter` and `lfilter` but not the new `pfilter`, so clearing filters on Persistence would leave the chip stuck -- fixed by adding `pfilter: 'all'` to that one shared reset; `psig(i)` re-derived the same apple/signed/unsigned three-way branch `sigKind(g)` already computes, instead of reusing it -- fixed to call `sigKind` and only special-case its `'unchecked'` leaf into `null` (the one place the two pages' Trust semantics actually differ: Persistence needs "-" not counted as unsigned, the Apps page's signing filter needs a distinct "unchecked" option). Both fixes reverified against the real browser, including a direct check that `clearFilters()` resets `S.pfilter` to `'all'`.
- **Deviation:** none from the sketch beyond the two review fixes above.
- **Open:** none. Disabling/removing a startup item and login items are explicitly out of scope (a later todo, Risks section).
- **CI:** pending on the pull request.
