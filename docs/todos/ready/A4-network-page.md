# A4: Network page

| Field | Value |
|---|---|
| ID | `A4` |
| Status | `ready` |
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

- [ ] Group outbound connections by `group.key` + `remote_addr`; join `seen` rows by `group_key` and `remote`.
- [ ] Show "-" for first seen when no row exists yet (history is recorded by the server every 10 s).
- [ ] Sort by first seen descending, then sockets.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
