# B3: Per-process usage table

| Field | Value |
|---|---|
| ID | `B3` |
| Status | `done` |
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

- [x] Write the rate tests first (recycled pid, counter reset).
- [x] Implement; cache nothing across pid reuse.
- [x] Record the cost of one full pass in the Implementation Record.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/procusage.rs` -- `Tracker::sample` reads `collect::processes()`'s own list every 5 s, skips straight to `ps`-derived CPU/memory for another user's process (never attempting `proc_pid_rusage`, documented to always fail for one, so no wasted syscall), and for your own processes calls `proc_pid_rusage` (V4 first, has disk I/O; V0 as a non-permission-error fallback), converting `ri_user_time`/`ri_system_time` from mach absolute-time ticks to nanoseconds via a real `mach_timebase_info` call. Rates are diffed against the previous sample, looked up through one named function (`prev_for`) that requires both the pid *and* `ri_proc_start_abstime` to match -- a recycled pid is never diffed against whatever used to have it. `GET /api/processes?sort=cpu|mem|disk&limit=` in `src/server.rs`, sorting and truncating at request time from a shared `Arc<RwLock<Vec<ProcessUsage>>>` the background thread writes to every 5 s. `actions::my_username()` added alongside the pre-existing `my_uid()`.
- **Tests:** 106 Rust tests pass (100 before; +6 in `procusage.rs`: a first sample reports memory but no CPU rate; a real two-sample CPU/disk rate calculation; a recycled pid tested directly against `prev_for` itself, not a copy of its logic; the V0 fallback correctly reporting disk as `Unavailable`, not `0`; a restricted (another-user's) row using `ps`'s own cpu/rss with disk `Unavailable`; and `sort_and_limit`'s three sort modes). A `#[ignore]`d real-machine timing test (`real_full_pass_timing_on_this_mac`) reproduces the cost-of-one-pass figure below on demand.
- **Cost of one full pass on this Mac:** 616-624 processes, `collect::processes()` (the `ps` call) ~64 ms, the `proc_pid_rusage` pass itself ~1.6-1.7 ms, total ~65 ms -- comfortably inside the 5 s budget, dominated entirely by the pre-existing `ps` shell-out, not by anything new this todo added.
- **Real run and comparison against `ps`/Activity Monitor's own data source** (two separate runs): the first, under light load, showed strong overlap in which processes were busiest (WindowServer, mds_stores, runningboardd, Ghostty, a Claude Helper process, TextEdit all appeared at the top of both this tool's `/api/processes?sort=cpu` and `ps aux -r`), with the actual percentages in the same range but not identical -- expected, since `ps`'s `%CPU` is an exponentially-decayed average over the process's whole recent history, not this tool's precise rate over exactly the last 5 s. The second run, caught during a heavy parallel `cargo`/`rustc` build on this machine, found a real and informative limitation: several individual `rustc` invocations that `ps -r` showed consuming 30-180% CPU did not appear in this tool's output at all, because they started and exited entirely within a single 5 s gap between samples -- the same "short-lived processes can be missed between polls" limitation this project's own ADR 0002 already names for the whole polling architecture, not a defect specific to this todo. A longer-lived `rustc` process that survived across a sample boundary did appear, with a plausible (81%) rate. Recorded honestly rather than only reporting the favorable first comparison.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: a comment citing "the todo's own edge case" as its justification -- the same AGENTS.md rule 9 mistake caught in `iokit.rs`'s review and meant to not recur, reworded to state the user-facing reason directly; a SAFETY comment that covered two separate `unsafe` blocks (leaving the first, `std::mem::zeroed()`, looking uncommented) and mixed a safety invariant together with an unrelated "why this crate" design note -- split into two comments, one per block, with the design note moved above the function instead; the fragile `libproc` error-text matching ("Permission"/"EPERM"/"EACCES") was left undocumented as a risk -- a comment now flags it explicitly, since there is no typed alternative `libproc` exposes; the `recycled_pid_is_a_new_process_not_diffed_against_the_old_one` test didn't actually call any production code, only a hand-copied `.filter(...)` matching its logic -- the real decision was extracted into a named `prev_for` function and the test rewritten to call it directly; and two near-identical early-return branches in `build_usage` both used the same `"not sampled yet"` reason for two different situations (truly first-ever sample, vs. a real previous sample at the exact same instant) -- deduplicated into one `no_rate(reason)` closure with a distinct, accurate reason for each case.
- **Deviation:** none from the sketch beyond the fixes above.
- **Open:** none beyond the already-accepted, architecture-wide "missed between polls" limitation noted in the real-run comparison above.
- **CI:** pending on the pull request.
