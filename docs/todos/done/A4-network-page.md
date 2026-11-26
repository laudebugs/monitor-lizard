# A4: Network page

| Field | Value |
|---|---|
| ID | `A4` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a4-network-page` |
| PR title | `feat(web): add the network page with first-seen dates` |
| Size | M (200-400) |
| PR | — |

## Goal

Replace the Network placeholder with outbound connections grouped by app and remote host, showing ports, scope, socket count and when the host was first seen for that app.

## Context

- `/api/history` `seen` rows (from [A1](../done/A1-backend-persistence-history-api.md)) hold `first_seen`, `last_seen`, `hits` per app and remote.
- Reverse DNS is deliberately out: it tells a resolver which IPs you look up (plan section 5).

## Scope

**In scope:**

- Rows: app, remote address, ports, scope (public/LAN), sockets now, first seen.
- Chips: All, Public, LAN, New (first seen in the last hour for an approved app).

**Out of scope** (name the todo that covers it):

- Hostname or owner enrichment: needs an opt-in design.
- Blocking a host: needs the network extension.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove `network` from `SOON`; add `network()` view, `loadHist` (every 30 s via `quiet`) |

## Steps

- [x] Group outbound connections by `group.key` + `remote_addr`; join `seen` rows by `group_key` and `remote`.
- [x] Show "-" for first seen when no row exists yet (history is recorded by the server every 10 s).
- [x] Sort by first seen descending, then sockets.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Remote seen only in history, not now | Not listed on this page (History page covers it) |
| IPv6 remote with brackets | Displayed without breaking the column |
| App with 100+ remotes | Paginated |

## Acceptance criteria

- Every row's socket count matches the snapshot.
- First-seen dates match `/api/history`.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

jsdom smoke test as in A2.

## Verification

```sh
make check\nmake test
```

## Docs to update

- Plan section 2

## Risks and open questions

- Hosts are IP addresses only; say so in the page copy.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose beyond one short page footer note, following the existing pattern in listeners(); not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `network()` view in `web/index.html` -- groups live outbound connections by `group.key` + `remote_addr` (via a shared `netKey()` helper used at every build/join/lookup site, so the key shape can't drift), joins against `/api/history`'s `seen` rows for first-seen dates, with chips (All, Public, LAN, New), a sortable table (App, Remote, Ports, Scope, Sockets, First seen), search on label and remote, and `loadHist()` polling `/api/history` every 30 s. The default order (newest-contacted first, then by socket count) is baked into the base row list before filtering, the same way `listeners()` bakes in its reach-tier order, rather than bolted on as a separate post-sort step.
- **Real run on this Mac:** against live data, 70-87 real outbound rows across a session (count naturally fluctuates with live connections), correctly split Public/LAN, with real IPv6 remotes (bracketed, e.g. `[2607:f8b0:4004:c27::6c]`) rendering without breaking the column.
- **Real-browser verification:** no Claude-in-Chrome extension was connected this session, so headless Chrome was driven directly over the DevTools Protocol (a local WebSocket). Checked: every chip's rendered row count matches its own printed count (capped at page size); a row's displayed socket count cross-checked directly against `S.snap`'s real connection count for that app/remote pair; every rendered row's first-seen date cross-checked against `S.seen` (`/api/history`) exactly, all 25 matched; search filters in place without jumping views; IPv6 cells use `overflow:hidden` plus a `title` attribute with the full address. Layout checked visually at 1440 px and 1000 px (below the todo's 1100 px threshold).
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: a code comment citing "the todo's Steps" as its justification instead of the user-facing reason (AGENTS.md rule 9) -- reworded to explain why newest-first ordering is useful; the composite `groupKey + '|' + remote` key was built separately at three call sites with no shared helper -- extracted into one `netKey()` function, also turning a `has()` + `get()` double lookup into one `get() ?? null`; the default sort was bolted onto `sortRows`'s output with a separate `if (!S.sorts.net)` check instead of being baked into the base row order the way `listeners()` does it for its reach tiers -- restructured to match that convention, which also makes the true default order (first-seen desc, then sockets) the one `sortRows` itself falls back to, rather than a parallel path that could drift from it. Also fixed, from the spec review: loopback-scope outbound connections had no chip to isolate or exclude them and would have shown up under "All" with no way to filter them out -- excluded at the row-building stage, consistent with `apps()`'s own `net(g)` helper, which already treats non-loopback as the definition of "uses the network." All fixes reverified against the real browser.
- **Deviation:** none from the sketch beyond the review fixes above.
- **Open:** none. Hostname/owner enrichment and blocking a host are explicitly out of scope (noted in the page's own footer copy and in Risks below).
- **CI:** pending on the pull request.
