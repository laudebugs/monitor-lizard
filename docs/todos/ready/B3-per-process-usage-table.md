# B3: Per-process usage table

| Field | Value |
|---|---|
| ID | `B3` |
| Status | `ready` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b3-per-process-usage-table` |
| PR title | `feat(metrics): add per-process CPU, memory and disk I/O to the sampler` |
| Size | M (200-400) |
| PR | — |

## Goal

Every 5 s, read CPU, memory footprint and disk I/O for each of your own processes with `proc_pid_rusage`, compute rates from diffs keyed by pid and start time, and serve the top rows over `/api/processes`.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- `ri_user_time` and `ri_system_time` are in mach absolute-time units; `ri_phys_footprint`, `ri_diskio_bytesread` and `ri_diskio_byteswritten` are the fields we need (V4 and later).
- Other users' processes fail with a permission error: report them as restricted, using `ps` for CPU and memory only.

## Scope

**In scope:**

- `src/procusage.rs`: rusage read, `mach_timebase_info` conversion, rate calculation keyed by (pid, start time).
- `GET /api/processes?sort=cpu|mem|disk&limit=` returning rows with a `restricted` flag.

**Out of scope** (name the todo that covers it):

- The page: B8.
- Per-process GPU and energy: not planned.

## Target files

| File | Change |
|---|---|
| `src/procusage.rs` | (new) |
| `src/server.rs` | route |
| `docs/agents/layout.md` | add the file |

## Steps

- [ ] Write the rate tests first (recycled pid, counter reset).
- [ ] Implement; cache nothing across pid reuse.
- [ ] Record the cost of one full pass in the Implementation Record.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Pid recycled between samples | Treated as a new process; no rate computed against the old one |
| Process exits mid-read | Row skipped |
| Another user's process | `restricted: true`, CPU and memory from `ps`, disk "-" |
| Rusage version unavailable | Fall back to a lower version and mark disk "-" |

## Acceptance criteria

- `/api/processes?sort=cpu` lists plausible top processes on this Mac, matching Activity Monitor within a reasonable margin (record the comparison).

## Tests

Rust tests for rate maths and pid reuse (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- Per-process history is sensitive: it stays in memory only (plan decision D3).

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
