# B6: Memory detail page

| Field | Value |
|---|---|
| ID | `B6` |
| Status | `done` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b6-memory-detail-page` |
| PR title | `feat(web): add the memory detail tab` |
| Size | M (200-400) |
| PR | — |

## Goal

The Memory tab: stacked area of App, Wired, Compressed and Cached, a pressure card colored on pressure level with the percentage available, and a swap bar.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- One report says the pressure level stayed "normal" during swap thrash, so always show the available percentage next to the level [U].
- The "used = app + wired + compressed" mapping is inference: compare with Activity Monitor and record the result.

## Scope

**In scope:**

- Stacked chart, pressure card (level tag and percent), swap card, explanatory copy as in the design.

**Out of scope** (name the todo that covers it):

- Per-process memory: B8.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Memory tab |

## Steps

- [x] Build from the design.
- [x] Record the side-by-side with Activity Monitor.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Pressure level 4 | Bad tag |
| Swap total 0 | Swap card says "no swap" instead of dividing by zero |
| Pressure unreadable | Unavailable with the reason |

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
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `web/index.html` only. `MemorySample` (`src/sampler.rs`) already carried every field the tab needed (app/wired/compressed/cached bytes, total, swap used/total, pressure level and percent available), so no Rust changes were required. Memory became the Performance page's fourth internal tab (`S.perfTab`, alongside `overview`/`cpugpu`/`disknet`): `perfMemoryTab_()` renders three cards — a four-layer stacked area (App/Wired/Compressed/Cached) with a legend and the inference caveat from Context, a pressure card (a colored tag from level 1/2/4 plus the percent-available line, shown even when the level reads "normal"), and a swap card using the same capacity-bar styling as the Disk tab. The Memory tile on the Overview tab, previously inert, now opens this tab like every other tile.
- **`stackedArea()` generalized:** the existing two-layer stacked-area chart (built for B5's CPU/GPU tab) only supported exactly two series, so it couldn't draw Memory's four layers directly. Generalized it into `stackedAreaN(layers, max, hgt)` for any number of `{values, color}` layers, keeping the same cumulative-stacking math; `stackedArea(bottom, top, max, hgt)` is now a one-line call into it with the CPU/GPU tab's original two colors, so B5's chart is unchanged in behavior (confirmed by the existing CPU/GPU tab still rendering correctly after the change).
- **Tests:** added `tests/web/memory-tab.test.mjs` (6 cases) covering the three edge cases the todo lists as pure logic, extracted from a new `memory-tab` block: pressure level 4 is a bad tag, levels 1/2 are normal/warning, an unrecognized level reads as "Unknown" rather than a guessed severity; swap total 0 reads as "no swap" rather than a divide-by-zero; swap is "unavailable" when either reading is missing; swap-in-use percentages are capped at 100. `make test` is 191 (was 185).
- **Side-by-side with Activity Monitor:** opened Activity Monitor (no scripted UI access was available in this session, so its window could not be screenshotted alongside the page), and instead compared the sampler's formula directly against the raw OS readings Activity Monitor itself is built on: ran `vm_stat` and `sysctl vm.swapusage hw.memsize kern.memorystatus_vm_pressure_level kern.memorystatus_level` on this Mac and hand-computed the same formula `src/sampler.rs` uses (active+inactive+speculative-purgeable-external for App, wired_count for Wired, compressor pages for Compressed, external pages for Cached, all times the page size). The hand-computed figures (App 4.26 GB, Wired 2.44 GB, Compressed 7.18 GB, Cached 2.59 GB, swap 10.5 of 11.81 GB, pressure level 2/41%) matched the running app's own reading within the few seconds of drift between the two reads. This confirms the "used = app + wired + compressed" mapping against the same source Activity Monitor reads, though not a literal side-by-side screenshot of its own window; that remains open if a closer comparison is ever wanted.
- **Real run and real browser:** built the release binary, ran it against a live server, and drove it through the Claude in Chrome extension. Verified live: the stacked chart renders four distinct, correctly-ordered layers (App at the bottom through Cached at the top) with legend values tracking `/api/metrics`; the pressure tag changed from NORMAL to WARNING live as the real pressure level changed between polls, with the percent-available line always present; the swap bar's fill width and color track the real used/total swap. Checked at the default window width (**1375px**, above the 1100px breakpoint) and at **1000px** (below it) — both wrap the tab/window-toggle controls onto two rows as the rest of the Performance page does, no overlap or clipping, dark theme (the only theme exercised this session).
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: two sentences over the docs guideline's 40-word limit (split in two); a repeated "first unreadable field's reason" shape duplicated between the Overview tile's memory reason and the new tab (factored into a shared `firstReason()` helper); the four memory values travelling together as separate same-shaped parameters, including one case zipping two parallel arrays by index (bundled into one `mem` object and one `MEMORY_LAYERS` list keyed by field name, removing the zip). Spec findings fixed: the pressure card's "unreadable" state read as a bare reason string, unlike the swap card two lines below it in the same tab, which explicitly says "Swap is unavailable: ..." — the spec's own edge-case row ("Pressure unreadable | Unavailable with the reason") asks for the word "Unavailable," so the pressure tag and percent-available line now both say so explicitly, matching the swap card's wording. The spec review also caught a process problem the review step itself exists to catch: an earlier checkpoint commit had moved this file straight from `ready/` to `done/` with every box still unchecked and no Implementation Record; this is fixed by splitting the work into a claim commit (`ready/` -> `ongoing/`) followed by this one, which fills in the record and makes the final `ongoing/` -> `done/` move.
- **Writing review:** the two new UI sentences (the Activity Monitor inference note) were checked against the 5Cs: both are active voice, under 30 words, and the point (what the mapping is, and that it's unverified) leads each sentence rather than trailing a caveat.
- **Deviation:** none beyond what Context already called for (the inference caveat, the always-shown percentage). `docs/agents/layout.md` was updated for the new `memory-tab` test block, matching the existing entry for `disk-net`.
- **Open:** per-process memory stays out of scope here, covered by B8. A literal side-by-side screenshot against Activity Monitor's own window is still open if the numeric cross-check above isn't considered sufficient.
- **CI:** pending on the pull request.
