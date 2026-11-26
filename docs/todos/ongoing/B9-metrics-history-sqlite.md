# B9: Metrics history in SQLite (optional)

| Field | Value |
|---|---|
| ID | `B9` |
| Status | `ongoing` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b9-metrics-history-sqlite` |
| PR title | `feat(metrics): persist downsampled metrics history` |
| Size | M (200-400) |
| PR | — |

## Goal

Optionally keep downsampled system metrics (never per-process) in SQLite so the 1 hour window can grow to days: 2 s for 1 hour, 1 minute for 48 hours, 15 minutes for 30 days, averages plus maxima for CPU and memory.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Plan decision D3: per-process history is never written to disk by default.

## Scope

**In scope:**

- `samples(ts, res, metric, v)` with `PRIMARY KEY(res, metric, ts)`, WAL mode, batched writes every 30 s, roll-ups and pruning.

**Out of scope** (name the todo that covers it):

- Per-process history: not planned.

## Target files

| File | Change |
|---|---|
| `src/history.rs` | tables, writer, roll-up, prune |
| `src/server.rs` | `/api/metrics?res=` |

## Steps

- [ ] Write roll-up and prune tests first.
- [ ] Implement behind a setting that defaults to off.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Process killed mid-batch | At most 30 s lost; database intact (WAL) |
| Roll-up across a gap | Gap stays a gap, no interpolation |
| Setting off | Nothing written |

## Acceptance criteria

- With history on, a restart keeps the last day of charts.

## Tests

Rust tests (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- README (the data folder holds more now)

## Risks and open questions

- Disk use: measure growth over a day and record it.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
