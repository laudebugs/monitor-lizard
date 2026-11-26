# B7: Disk and network detail page

| Field | Value |
|---|---|
| ID | `B7` |
| Status | `ready` |
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

- [ ] Build from the design.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
