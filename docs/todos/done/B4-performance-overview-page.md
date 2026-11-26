# B4: Performance overview page

| Field | Value |
|---|---|
| ID | `B4` |
| Status | `done` |
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

- [x] Add the loader and a client ring buffer of the same length as the server's.
- [x] Build the sparkline and tile components against the design.
- [x] Add unavailable and restricted states with their reasons.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose beyond short tile labels; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `web/index.html` only (no Rust changes; `/api/metrics?since=` already existed from B1). A `performanceView_()` view polls it every 2s via `loadMetrics`, gated to run only while `S.view === 'performance'`, `S.perfLive` is true and the tab isn't hidden; `visibilitychange` re-uses the same function so a backgrounded tab gets exactly its gap in one request on return, never a burst. The client ring buffer is capped at the server's own `RING_CAPACITY` (1800, mirrored as a named constant with a comment pointing at `src/sampler.rs`). Six tiles: CPU, GPU, Memory, Disk, Network each with a sparkline and a real, accurate `aria-label`; Power and thermals permanently `Unavailable` (no power-reading code exists anywhere in this codebase, not just gated on administrator rights this moment). 5 min / 1 hour window; 1h downsampled to about one point per two pixels of the sparkline's actual measured width (read from the previous render's own element), not a hardcoded assumption. Each numeric tile also carries a `stale` state (dimmed, tagged) distinct from `na`/`Unavailable`, when no new sample has arrived in a while -- the three tile states the Goal names.
- **Tests:** no new automated tests (this view has no pure logic module to unit test the way `sampler.rs`/`procusage.rs` do; `make test`'s existing 106 Rust tests are unaffected and still pass). Verified instead by direct inspection in a real, running instance (below), per the todo's own "jsdom smoke test... kept out of the repo" note.
- **Real run and real browser (headless Chrome via CDP, since no Claude-in-Chrome extension was connected this session):** ran the built release binary, confirmed `/api/metrics?since=` returns real `Reading<T>` JSON exactly as assumed. Verified in-browser: all five real tiles show plausible live values with working sparklines; Power and thermals shows the dashed `na` tile with its reason; Pause genuinely stops polling (metrics length unchanged across 4s with the button in the Paused state); the 1h/5m window toggle works and the 1h downsample target tracked the sparkline's real measured width (246px measured -> matching downsample count), not a fixed guess; switching to a different nav view stops the poll entirely (metrics length unchanged across 4.5s on Overview) and switching back to Performance re-fetches immediately. Layout checked at **1440px** and **1024px** (below the 1100px breakpoint named in the acceptance criteria) in both dark and light themes -- all four combinations screenshotted, tiles reflow from a wide grid to two columns below 1100px with no overlap or clipping, matching the sidebar/card language the rest of the app (and the approved `Perf.dc.html` design board) already uses.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: a comment citing "the todo's edge case" (AGENTS.md rule 9, the same mistake caught repeatedly elsewhere this session); the todo's own Status field had silently stayed `ready` after claiming (a prior shell command's `python3` step threw on a backtick-escaping bug and the following `git mv`/commit ran anyway since nothing in that block used `&&` to stop on failure -- the folder move happened but the Status text inside the file never did; fixed here, and a lesson for this session: chain file-editing steps with `&&` or check exit codes, don't assume a multi-line bash block halts on a failed step); a stale `todos/ready/...` link for B1-B4 in `docs/plans/resource-monitoring.md` (now pointing at `done/`, matching where those four todos actually live). Judgement calls addressed: extracted a repeated `S.perfWin === '1h' ? 'hour' : '5 min'` ternary into one `winLabel`; replaced the label-keyed `reasonFor` lookup map with an explicit reason thunk passed alongside each tile's own data, removing the untyped coupling between a tile's display label and its reason lookup; named and commented the `performance_` -> `performanceView_` trailing-underscore convention (avoids shadowing `window.performance`, the only such case in this file). Spec findings fixed: `prefers-reduced-motion` was never actually respected (nothing animates today, but the page polls and redraws indefinitely, so a reduced-motion media query is added now rather than left for whoever adds the first transition to remember); the restart-detection comment made a false claim (`src/sampler.rs` stamps every sample with the real wall clock, not a clock that resets on restart, so a restart's samples simply continue counting forward from "now" -- the comment is corrected to describe the narrower, real guarantee: an out-of-order ts, which a restart can never produce but a corrected system clock could, is what actually triggers the client to drop its old buffer); "stale" was only a page-level indicator, not a per-tile state as the Goal's "three tile states" implies -- added `.tile.stale` with a dimmed look and a `stale` tag per numeric tile; the 2s poll ran regardless of which nav view was active -- gated to the Performance view, with an immediate catch-up fetch wired into that nav button's own click handler rather than waiting for the next 2s tick; the 1h downsample's "one point per two pixels" assumed a hardcoded 200px width that the actual responsive `.spark{width:100%}` never guaranteed -- now measured from the DOM.
- **Deviation:** none from the sketch beyond the fixes above.
- **Open:** detail tabs (CPU and GPU, Memory, Disk and network) and the Top consumers table are out of scope here by design, covered by B5/B6/B7/B8.
- **CI:** pending on the pull request.
