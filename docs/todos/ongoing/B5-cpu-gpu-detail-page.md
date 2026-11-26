# B5: CPU and GPU detail page

| Field | Value |
|---|---|
| ID | `B5` |
| Status | `ongoing` |
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

- [ ] Confirm the cluster mapping on this Mac and record it.
- [ ] Build the tab from the design.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
