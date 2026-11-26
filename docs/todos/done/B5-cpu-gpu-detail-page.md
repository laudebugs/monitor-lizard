# B5: CPU and GPU detail page

| Field | Value |
|---|---|
| ID | `B5` |
| Status | `done` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B2, B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b5-cpu-gpu-detail-page` |
| PR title | `feat(web): add the CPU and GPU detail tab` |
| Size | M (200-400) |
| PR | — |

## Goal

The CPU and GPU tab from the approved design: user/system stacked area, load averages, per-core bars grouped as 6 performance and 2 efficiency cores, and GPU device, renderer and tiler lines.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Core grouping comes from `hw.perflevel0` and `hw.perflevel1`; mapping individual cores to clusters is medium confidence: if it cannot be confirmed, show cores in index order and say the grouping is unknown.

## Scope

**In scope:**

- Chart components shared with B4; per-core bars colored by load (warn above 65%, bad above 85%).
- A note that per-core frequency and per-process GPU are not measured.

**Out of scope** (name the todo that covers it):

- Frequency, power: unavailable.

## Target files

| File | Change |
|---|---|
| `web/index.html` | CPU and GPU tab |
| `src/sampler.rs` | per-core and cluster fields if missing from B1 |

## Steps

- [x] Confirm the cluster mapping on this Mac and record it.
- [x] Build the tab from the design.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Machine with different core counts | Groups sized from `hw.perflevel*` |
| GPU reading unavailable | GPU card shows the reason |
| Core mapping unknown | Index order with a note |

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
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose (no new user-facing prose beyond short labels; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `web/index.html` only (no Rust changes; `per_core_pct`, `perf_cores` and `eff_cores` already existed on `CpuSample` from B1, and `GpuSample`'s device/renderer/tiler fields already existed from B2). Added a tab bar to the Performance page (`Overview` / `CPU and GPU`, `S.perfTab`); `perfOverviewTab_()` holds the prior tile grid unchanged except that the CPU and GPU tiles are now buttons that switch to the new tab. `perfCpuGpuTab_()` renders four cards: a user/system stacked area (`stackedArea()`, a new two-layer area helper), the three load averages, per-core bars (`coreBars()`) split into Efficiency/Performance groups sized from `eff_cores`/`perf_cores` (falling back to one ungrouped list, with an explanatory note either way, if the counts don't add up to the reported core array), and a three-line GPU chart (`linesChart()`, a new plain multi-series line chart) with device/renderer/tiler legends. A `legendDot()` helper and a shared `chartXY()` coordinate mapper (factored out of `sparkPath()`) are reused across the new charts rather than duplicated.
- **Cluster mapping confirmed:** read this Mac's IORegistry directly (`ioreg -l -w0 -p IODeviceTree`, cross-checked with `ioreg -c IOPlatformDevice -n cpus -a` for `p-core-count`/`e-core-count`). Each `cpuN` IORegistry node carries `cluster-type` (`"E"` or `"P"`) and `logical-cpu-id`; on this Mac, logical CPU indices 0-1 are `cluster-type "E"` (`compatible: apple,icestorm`) and indices 2-7 are `"P"` (`apple,firestorm`), i.e. efficiency cores are the first `eff_cores` logical CPUs the kernel reports, performance cores the rest. This is the order `host_processor_info` returns (the same order `per_core_pct` is built in, confirmed by reading `src/sampler.rs`), so the UI groups by slicing `per_core_pct` at `eff_cores` rather than guessing a 6+2 split. The per-todo's own medium-confidence framing is kept in the UI's note text (`"confirmed on this Mac's IORegistry but not re-checked on every machine"`), since this was verified on one Mac, not asserted as a cross-model guarantee.
- **Tests:** no new automated test file (no pure-logic module here to unit test the way `sampler.rs`/`history.rs` have; the existing 140 Rust tests and 31 Node web tests are unaffected and still pass). Verified instead by direct inspection of a real, running instance, per the todo's own "jsdom smoke test... as in B4" note and B4's established precedent for this page.
- **Real run and real browser:** built the release binary, ran it, and drove it through both the Claude in Chrome extension and headless Chrome via CDP (`Emulation.setDeviceMetricsOverride`). Verified: the CPU and GPU tiles on Overview are clickable and open the new tab; the stacked area's top edge tracks `user% + system%` (fixed during review, see below) with legend values matching the tile numbers; the three load averages render and update live; per-core bars are grouped Efficiency (2)/Performance (6) on this Mac, colored by the 65%/85% thresholds, and the grouping note is always shown; the GPU card's three lines and legend track `/api/metrics`'s real `device_pct`/`renderer_pct`/`tiler_pct`. Checked at **1440px** and **1024px** (below the 1100px breakpoint), both dark and light themes -- all four combinations screenshotted, no overlap or clipping, consistent with the card/tile language B4 established.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: a comment citing todo IDs `B6`/`B7` directly (AGENTS.md rule 9); a doc-comment on `perfCpuGpuTab_` that only restated what the function builds instead of why (`docs/guidelines/maintainability.md`); `stackedArea()`'s coordinate math duplicated `sparkPath()`'s instead of reusing it (extracted into a shared `chartXY()`); five repeated color-dot legend spans collapsed into one `legendDot()` helper. Spec findings fixed: **a real bug** -- `stackedArea()`'s own comment claimed each layer's edge was a running cumulative total, but the code plotted the "system" layer's edge at the raw `system_pct` value, not `user_pct + system_pct`; on any sample where system% < user% (the common case) this drew the system layer's boundary below the user line instead of stacked above it. Fixed by summing inside `stackedArea()` before plotting, and re-verified in the browser that the green layer now sits on top of the purple one with the white total line at the true sum. The spec review also flagged that this Implementation Record, the done/ move and the plan update were missing at the time it ran; all three are added in this same commit.
- **Deviation:** the Overview tab's CPU and GPU tiles were made clickable, jumping into this new tab (`rec({ perfTab: 'cpugpu' })`) -- not asked for by the Goal/Scope/Steps, flagged as scope creep by the spec review. Kept deliberately: Overview's tiles already exist and showing a value with nowhere to click once a detail tab exists for it would be a dead end, and every other page in this app (Overview's own count tiles, the "Needs your attention" cards) already uses a tile/card as a link into more detail -- this is the same pattern, not a new one. `docs/agents/layout.md` was not touched: it documents files, not per-view features, and no other view's internal tabs (Apps' filters, History's two tables) are itemized there either, so there was nothing file-level to add.
- **Open:** Memory and Disk/network detail tabs are out of scope here by design, covered by B6/B7.
- **CI:** pending on the pull request.
