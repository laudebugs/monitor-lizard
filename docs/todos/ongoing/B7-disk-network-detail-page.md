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
| Size | M (200-400) |
| PR | — |

## Goal

The Disk and network tab: read and write throughput with operations per second, a capacity bar (used, purgeable, free) and network download and upload charts.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)

## Scope

**In scope:**

- Charts and the capacity bar from the design; capacity for the boot volume.

**Out of scope** (name the todo that covers it):

- Per-process disk I/O: B8.
- Per-interface network breakdown: not planned.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Disk and network tab |

## Steps

- [x] Build the tab (the design itself was not available to check against; see the record).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Purgeable space unavailable | Bar shows used and free only, with a note |
| No network interface up | Network card shows "no connection" |

## Acceptance criteria

- Matches the approved design.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

jsdom smoke test as in B4.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- None known.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26. Status: left in `ongoing/`; two acceptance items are open (below).

- **Built:** a "Disk and network" tab in the Performance tab bar, in the same way as the CPU and GPU tab, with no new NAV entry. It has a capacity card for the startup disk, a read and write throughput chart, an operations-per-second chart and a network download and upload chart. The Overview Disk and Network tiles now open it.
- **Reusable bar:** `capacityBar(args, label)` in `web/index.html` takes `{ used, purgeable, free, unavailable }` (bytes, or `null` for a part that could not be read, plus the reason). The sizing is `capacitySegments`, in the `disk-net` block that `tests/web/disk-net.test.mjs` loads.
- **Reused from the storage scanner:** `volume::split_capacity`, `iokit::boot_volume_capacity` and `volume::available_and_important`. `GET /api/volume` (`storage::boot_capacity_json`) returns that capacity without starting a folder scan, which `/api/storage` would do on its first request.
- **Added to the sampler:** `network.interfaces_up`, from `src/netif.rs`. The byte counters could not separate "no connection" from "idle": a down interface and a quiet one both give a rate of zero. `interfaces_up` of 0 shows "No connection"; `null` (list unreadable) shows "unavailable".
- **Tests:** 9 Node cases in `tests/web/disk-net.test.mjs` (bar sizing, purgeable unavailable, no used or free reading, rate formatting, no connection versus unavailable, chart top); Rust tests for `count_active` and for the `/api/volume` reply shape.
- **Browser:** Chrome, Performance page, own tab, scratch HOME. Seen: the live tab in dark (about 1375 px wide) with a real capacity split; purgeable unavailable (used and free only, with the note); no connection (set by editing page state, not by unplugging); light theme; a 400 px same-origin frame with no horizontal overflow and four cards fitting.
- **Open:** (1) "Matches the approved design": the design canvas was not available to this session, so the layout follows the CPU and GPU tab and is not compared with the design. (2) The 1440 px and below-1100 px checks: the window could not be resized, so widths of about 1375 px and 400 px were looked at, not 1440 or 1000.
- **Known gap:** the Overview Network tile still shows `0.0 MB/s` with no connection; it does not read `interfaces_up`.
