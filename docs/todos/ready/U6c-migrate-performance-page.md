# U6c: Migrate the Performance page

| Field | Value |
|---|---|
| ID | `U6c` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U6b, B7 |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6c-performance-page` |
| PR title | `refactor(web): run the Performance page on the Preact path` |
| Size | M (200-400) |
| PR | — |

## Goal

Performance is the live page: charts every 2 seconds, five tabs, 129 `h` calls and three chart builders that write `svg` bodies. On the Preact path an update should cost under 1 ms instead of rebuilding about 500 elements.

## Context

- Spike (d): Top consumers 32.2 ms rebuild vs 0.7 ms update; Overview 3.5 ms vs 0.5 ms.
- `web/page-performance.js`; `loadMetrics` uses `quiet()` and the live toggle.
- B7 is `ongoing` and edits this file: wait for it.

## Scope

**In scope:**

- Sparkline, multi-line and stacked charts through `svgBody`; keys on process rows; the live toggle keeps focus; `S.perfLive`, hidden-tab and view checks unchanged.

**Out of scope** (name the todo that covers it):

- Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-performance.js` | keys, chart sizing read from state |
| `tests/web/*.test.mjs` | markup helpers |

## Steps

- [ ] Open each tab for 60 seconds on the Preact path; the charts update, the page does not flicker, and a focused control stays focused.
- [ ] Measure update and full-rebuild time as in the plan and record the numbers.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Hidden tab, then visible | one catch-up, no burst |
| Live toggle off | polling stops; focus stays on the toggle |
| Metrics `unavailable` values | the existing "unavailable" text, never 0 |
| Window width change | chart width recomputed (sizes come from state or `ResizeObserver`, not a live DOM read in the builder) |
| 200 process rows | page size caps the DOM |

## Acceptance criteria

- Five tabs match the old path; update time is recorded and not worse than the old rebuild.

## Tests

Markup cases; browser checks and timings recorded.

## Verification

```sh
make check
make test
```

## Docs to update

None unless a rule changes.

## Risks and open questions

- Chart widths are measured from the live DOM today; moving that to state changes first-paint behaviour.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR.
- **Why the page works at every step:** Old path remains the default.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
