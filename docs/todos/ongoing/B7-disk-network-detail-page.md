# B7: Disk and network detail page

| Field | Value |
|---|---|
| ID | `B7` |
| Status | `ongoing` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B2, B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b7-disk-network-detail-page` |
| PR title | `feat(web): add the disk and network detail tab` |
| Size | L (about 660 changed lines; not split because the tab, its data and its edge cases ship together) |
| PR | [#50](https://github.com/laudebugs/monitor-lizard/pull/50) |

## Goal

The Disk and network tab: read and write throughput with operations per second, a capacity bar (used, purgeable, free) and network download and upload charts.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- The design is the canvas board "Performance overview, live" and its siblings, cited as D7 in the [plan](../../plans/resource-monitoring.md). It was not available to the session that built this, so the comparison is open.

## Scope

**In scope:**

- Charts and the capacity bar from the design; capacity for the boot volume.
- `GET /api/volume`: the capacity alone, so the tab never starts a folder scan.
- `network.interfaces_up` in the sampler: byte counters read 0 for both a down and an idle interface, so the edge case "no network interface up" cannot be told apart without it.
- The Overview Disk and Network tiles open the tab, and the Network tile shows "No connection" by the same rule.
- The open Performance tab is kept in the address (`tab=`), so a reload and Back and Forward keep it.

**Out of scope** (name the todo that covers it):

- Per-process disk I/O: B8.
- Per-interface network breakdown: not planned.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Disk and network tab, `capacityBar`, the `disk-net` logic block, the `tab` address key |
| `src/netif.rs` | (new) counts interfaces that are up, running and hold a routable address |
| `src/sampler.rs` | `network.interfaces_up` |
| `src/server.rs` | One route line for `/api/volume` |
| `src/storage/volume.rs`, `src/storage/mod.rs` | The `/api/volume` reply, re-exported |
| `src/main.rs` | Registers the `netif` module |
| `tests/web/disk-net.test.mjs` | (new) logic tests for the block |
| `tests/web/url-state.test.mjs` | Cases for the `tab` key |
| `docs/agents/layout.md`, `docs/plans/resource-monitoring.md` | Layout rows; plan link to `ongoing/` |

## Steps

- [x] Build the tab (the design itself was not available to check against; see the record).
- [x] Add `/api/volume` and `network.interfaces_up`.
- [x] Keep the tab in the address through the `url-state` block.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Purgeable space unavailable | Bar shows used and free only, with a note |
| No network interface up | Network card shows "no connection" |
| Capacity read fails | The card says so and retries; it never stays on "not read yet" |
| A part that is NaN, negative or huge | Treated as unreadable; no bar with a NaN width |

## Acceptance criteria

- [ ] Matches the approved design (comparison still open).
- [ ] UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (not done: the window could not be resized).
- [x] Purgeable unavailable shows used and free with a note (seen in Chrome, and tested).
- [x] No interface up shows "No connection" on the tab and on the Overview tile (seen with page state set, and tested).
- [x] The tab survives reload and Back and Forward (seen in Chrome).
- [x] Unavailable values are never drawn as 0 (tested).

## Tests

The `disk-net` logic block is loaded by a Node test (`tests/web/disk-net.test.mjs`), and the address key by `tests/web/url-state.test.mjs`. Rust tests cover `netif` and the `/api/volume` reply. There is no jsdom test.

## Verification

```sh
make check
make test
make check-docs
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- "Up" rule: an interface counts only if it is not loopback, is up and running, and holds an IPv4 address outside 169.254/16 or an IPv6 address outside fe80::/10. A VPN with a routable address counts. A machine with only a self-assigned address reads as no connection. Checked on one Mac only.
- Before this change the tab was not in the address, and the CPU and GPU tab had the same gap. Both are fixed here.
- The approved design has not been compared with the result.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26. Status: left in `ongoing/`; three acceptance items are open.

- **Built:** a "Disk and network" tab in the Performance tab bar, like the CPU and GPU tab, with no new sidebar entry. It has a capacity card, a throughput chart, an operations chart and a network chart. The Overview Disk and Network tiles open it.
- **Reusable bar:** `capacityBar(args)` takes `{ used, purgeable, free, unavailable }`: bytes, or `null` for a part that could not be read, plus the reason. Its sizing is `capacitySegments`, in the `disk-net` block.
- **Reused from the storage scanner:** `split_capacity`, `boot_volume_capacity` and `available_and_important`. `GET /api/volume` returns that capacity without a folder scan, and answers 500 with a short error if the reply cannot be written.
- **Network rule:** `interfaces_up` counts interfaces that are not loopback, are up and running, and hold a routable address. Link-local-only interfaces such as VPN tunnels and Apple peer-to-peer links do not count. On the build Mac, only en0 counted and the other 27 were skipped, loopback included.
- **Failed reads:** a failed `/api/volume` read shows "Could not read capacity. Retrying." and retries on the next poll; the first paint says "Reading capacity...".
- **Tests:** Node cases for bar sizing, bad numbers, whole-number percentages, capacity status, rate formatting, the network rule and the `tab` key; Rust tests for `netif` and the reply.
- **Browser:** Chrome with a scratch HOME, own tab. Seen in dark and light: the live tab with real capacity, purgeable unavailable, no connection (page state set, not a real outage) and a 400 px frame without overflow.
- **Open:** the approved-design comparison, and the 1440 px and below-1100 px checks, because the window was not resized.
- **Known gap:** the card helper is used only by the new tab; the older CPU and GPU cards still carry their own copy of the markup.
