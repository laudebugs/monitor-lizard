# B4: Performance overview page

| Field | Value |
|---|---|
| ID | `B4` |
| Status | `ready` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b4-performance-overview-page` |
| PR title | `feat(web): add the performance overview with live tiles` |
| Size | M (200-400) |
| PR | — |

## Goal

Add the Performance page from the approved design: six tiles (CPU, GPU, Memory, Disk, Network, and Power and thermals shown as unavailable), sparklines, a 5 minute or 1 hour window, Pause and Resume, a "live" indicator, and the three tile states.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Approved design: board "Performance overview, live" and "Performance overview, light" in the owner's design canvas (2026-11-26). Tabs: Overview, CPU and GPU, Memory, Disk and network.
- Plan decisions D4, D5 and D7.

## Scope

**In scope:**

- Nav item "Performance"; polling `GET /api/metrics?since=` every 2 s, paused while `document.hidden`, with a catch-up fetch on `visibilitychange`.
- Inline SVG sparklines; each chart has `role="img"`, an `aria-label` with the current value, and respects `prefers-reduced-motion`.
- Unavailable tiles show the reason from the sample; stale data shows a stale indicator.

**Out of scope** (name the todo that covers it):

- Detail tabs: B5, B6, B7.
- The Top consumers table: B8.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Performance view, loader, sparkline helper, nav item |

## Steps

- [ ] Add the loader and a client ring buffer of the same length as the server's.
- [ ] Build the sparkline and tile components against the design.
- [ ] Add unavailable and restricted states with their reasons.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Tab hidden for 5 minutes | On return, fetch the gap and draw it; no burst of requests |
| Server restarted | Client detects a timestamp reset and clears its buffer |
| `Unavailable` reading | Dashed tile with the reason; never a 0 |
| Window switched to 1 hour | Series downsampled to about one point per two pixels |
| Reduced motion | No animated transitions |

## Acceptance criteria

- The page matches the approved design in dark and light.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

jsdom smoke test against a running server (kept out of the repo): the page renders, polls, pauses when hidden and shows an unavailable tile.

## Verification

```sh
make check
make test
cargo run --release -- serve
```

## Docs to update

- `docs/agents/layout.md` if files change

## Risks and open questions

- Layout can only be confirmed in a real browser.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
