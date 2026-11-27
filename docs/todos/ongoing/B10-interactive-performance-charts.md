# B10: Interactive Performance charts

| Field | Value |
|---|---|
| ID | `B10` |
| Status | `ongoing` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B5, B6, B7, B8 |
| ADRs | None |
| Branch | `feat/b10-interactive-performance-charts` |
| PR title | `feat(web): make the Performance charts interactive` |
| Size | M (200-400) |
| PR | — |

## Goal

A user hovering any chart on the Performance page (CPU/GPU, Memory, Disk and network, and any future sparkline) sees the exact value and timestamp at that point, with a visible marker on the line. No more estimating from the axis.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md): every chart today is a fixed-viewBox inline SVG path (`sparkPath`, `linesChart`, `stackedArea`/`stackedAreaN` in `web/index.html`), drawn once per poll with no pointer handling at all.
- No design for the exact tooltip/marker look exists in the owner's design canvas yet; this todo's Steps start by picking a minimal, consistent interaction rather than waiting on one, since the charts already share one drawing style across tabs.
- D7 (the approved Performance page layout) does not describe hover behavior one way or the other.

## Scope

**In scope:**

- One shared hover/tooltip mechanism added to the chart-drawing helpers (`sparkPath`, `linesChart`, `stackedArea`/`stackedAreaN`), not a one-off per tab. Moving the pointer over a chart shows the nearest sample's value(s) and its real timestamp (relative, e.g. "32s ago", per D14's convention), with a vertical guide line and a dot per series at that x position.
- Keyboard equivalent: Left/Right arrow steps the highlighted sample while a chart has focus, for the same information without a pointer.
- Leaving the chart (pointer or focus) returns it to its plain, un-annotated state.

**Out of scope** (name the todo that covers it):

- Zoom, pan, or changing the time window from the chart itself: the existing 5 minute/1 hour toggle stays the only way to change the window.
- Per-core bars (`coreBars`) and the swap/capacity bars: they already show their own instantaneous numbers and are not time-series line charts.

## Target files

| File | Change |
|---|---|
| `web/index.html` (or its split files once T2 lands) | Shared hover/focus logic for the chart helpers; tooltip/marker rendering |

## Steps

- [x] Add one pure function that, given a chart's series data and an x position (pointer or keyboard index), returns the nearest sample's index and values -- no DOM, so it can be tested the way `confirmable`/`evidenceLine` are.
- [x] Wire pointer move/leave and keyboard handling on each chart helper, reusing that one function.
- [x] Draw the guide line, per-series dot, and a small tooltip (value, series label, relative time) without resizing the chart or shifting other page content.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Chart with no samples yet (e.g. "collecting the first sample...") | No hover/tooltip; nothing to point at |
| Pointer between two sample points | Snaps to the nearer one, never interpolates a value that was not sampled |
| A `Reading::Unavailable`/restricted value at the hovered point | Tooltip shows the reason, not a blank or a zero |
| Keyboard focus without a pointer | Arrow keys move the highlighted sample the same way hover would |

## Acceptance criteria

- Hovering or arrow-keying any chart on CPU/GPU, Memory, and Disk and network shows the correct value(s) and a real timestamp for that exact sample.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending).

## Tests

jsdom smoke test for the nearest-sample pure function (the established pure-logic-block pattern, e.g. `tests/web/actions.test.mjs`), 4 cases minimum.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- No approved design exists yet for the exact tooltip/marker visual style; Steps pick one consistent with the existing card/legend language rather than waiting on the owner's canvas, and the owner can redirect it once seen.
- T2 (splitting `web/index.html` into separate files) may land before or after this todo; whichever lands second should expect to re-apply its chart-helper edits in the new file layout per `docs/agents/layout.md`.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `web/chart-hover.js` (new, loads before `page-performance.js`, listed in `SOURCES` in `src/web.rs`). Its pure `chart-hover` block holds these functions: `chartIndexAt` (a pointer snaps to the nearer sample, and a tie takes the newer one), `chartStep` (Left, Right, Home, End, stopping at the ends), `chartIndexForTs`, `chartAgo` ("32s ago") and `chartSample`. `chartSample` returns the rows per series, the reason for an unavailable value and the dot height. `chartHost` wraps a chart svg with a guide line, a dot per series and a tooltip. All three are absolutely positioned, so nothing resizes or shifts. `spark`, `linesChart` and `stackedAreaN` take an optional spec and call it. The CPU, GPU, Disk throughput, Operations, Network and Memory charts are keyboard-reachable; the Overview tile sparklines answer the pointer only, because the tile is already a button. The highlight lives in `CH` by chart id, so each poll's rebuilt chart re-applies it; the focused chart gets focus back through `data-req`. Only a key press speaks, through the existing `#announcer`.
- **Tests:** `tests/web/chart-hover.test.mjs` has 12 cases (the todo asked for 4), covering every edge case in the todo. `tests/web/theme-contrast.test.mjs` gained tooltip text, border and guide pairs plus a check that the tooltip reads only covered tokens and names Canvas, CanvasText and Highlight in forced-colors.
- **Real run:** Chrome, scratch server and HOME under `/private/tmp/ml-b10`, window not resized (1309 px wide). Seen: hover shows the guide, dots and tooltip on CPU/GPU, Memory, and Disk and network, in dark and light, at 5 min and 1 hour. The highlight survives the 2 s redraw. Arrow keys step, and focus stays on the chart across polls. Each key press makes one announcement. Escape clears the highlight and it stays cleared after a redraw. Tooltips cause no change in page or tile height. A first run showed a resting pointer undoing a key press after a redraw; fixed by ignoring pointer events without movement.
- **Deviation:** the todo stays in `ongoing/`. B7, a dependency, was left in `ongoing/` on purpose (four open boxes, PR #67 closed), and `make check-docs` refuses a done todo whose dependency is unfinished. Move B10 to `done/` once B7 is. B7's feature itself merged in #50.
- **Open:** below-1100px layout, forced-colors, reduced-motion and a screen reader were not run (no window resize allowed, no such modes available): pending. The unavailable-value tooltip is covered by unit tests only; the scratch Mac had every reading.
- **CI:** pending on the pull request.
