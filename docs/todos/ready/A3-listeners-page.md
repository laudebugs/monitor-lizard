# A3: Listeners page

| Field | Value |
|---|---|
| ID | `A3` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a3-listeners-page` |
| PR title | `feat(web): add the listeners page` |
| Size | S (under 200) |
| PR | — |

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
