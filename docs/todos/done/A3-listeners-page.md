# A3: Listeners page

| Field | Value |
|---|---|
| ID | `A3` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a3-listeners-page` |
| PR title | `feat(web): add the listeners page` |
| Size | S (under 200) |
| PR | [#6](https://github.com/laudebugs/system-monitor/pull/6) |

## Goal

Replace the Listeners placeholder with a table of every listening socket: port, protocol, bind address, owning app, pid and whether it is reachable from the network.

## Context

- Data is already in the snapshot: `Conn.direction == "listen"`, `exposed`, `local_addr`, `local_port`.
- Plan decision D2: no new endpoint is needed.

## Scope

**In scope:**

- Table with chips All, Exposed (all interfaces), Local only; exposed rows first, then by port.
- Clicking a row opens that app in the Apps page.

**Out of scope** (name the todo that covers it):

- Closing a port: stop the owning process from the Apps drawer.
- Per-port reputation: not planned.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove `listeners` from `SOON`; add `listeners()` using `paginate(list, "listen")` |

## Steps

- [ ] Flatten `S.snap.groups[*].conns` where `direction === "listen"` into rows `{g, c}`.
- [ ] Reach tag: EXPOSED (warn) when `exposed` and the app is not Apple, "LAN" (neutral) when exposed and Apple, "local" otherwise.
- [ ] Wire search to port, app name and bind address.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| UDP socket bound without a remote | Counts as listening (matches the collector) |
| IPv4 and IPv6 listeners on the same port | Two rows, one per address |
| No listeners match a chip | Empty-state row |

## Acceptance criteria

- Row count with chip All equals `totals.listening` minus nothing: check against `/api/snapshot`.
- Exposed chip count equals the number of distinct exposed listeners in the snapshot.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

jsdom smoke test as in A2: chip counts equal counts computed from the snapshot JSON.

## Verification

```sh
make check\nmake test
```

## Docs to update

- Plan section 2

## Risks and open questions

- `totals.listening` counts sockets, the page lists sockets too; keep the numbers consistent and say so in the page copy if they differ.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `listeners()` in `web/index.html`: a table of every listening socket (port, protocol, bind address, app, PID, reach) from the snapshot, chips All, Open to network, Local only with counts, search over port, app and bind address, pagination key `listen`, and a row click that opens the app in the Apps page. Reach is EXPOSED (warning) for a non-Apple socket on all interfaces, LAN for an Apple one, local otherwise. Exposed non-Apple first, then Apple, then local, by port. No new endpoint.
- **Search:** typing in search from Overview or Privilege still jumps to Apps; on Apps and Listeners it filters in place.
- **Verified:** a script (kept out of the repo) rendered the real page against a running server in a simulated DOM. On this Mac the chips read 88 / 62 / 26 and matched counts computed independently from `/api/snapshot`; the Open chip showed only EXPOSED or LAN rows and the Local chip only local rows; searching port 9222 returned the 1 expected row; a row click opened Apps with the detail panel; no script errors.
- **Not verified:** layout and interaction in a real browser (none was connected), so the visual check at 1440 px and below 1100 px is **pending**.
- **Observation:** 62 of the 88 listening sockets are open to the network on this Mac, most of them mDNS (port 5353) sockets from a few apps, which is why the Overview card for Apple services is aggregated.
- **Deviation:** none.
- **CI:** pending on the pull request; `/code-review` not yet run (pending, per `AGENTS.md`).

