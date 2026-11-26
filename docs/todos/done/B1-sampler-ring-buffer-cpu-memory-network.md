# B1: Sampler, ring buffer and the CPU, memory and network metrics

| Field | Value |
|---|---|
| ID | `B1` |
| Status | `done` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b1-sampler-ring-buffer-cpu-memory-network` |
| PR title | `feat(metrics): add the sampler thread and CPU, memory and network metrics` |
| Size | M (200-400) |
| PR | #16 |

## Goal

Add one sampler thread that every 2 s reads CPU (total, per core, user and system), load average, memory (app, wired, compressed, cached, swap, pressure) and network throughput through system calls, keeps one hour in a ring buffer and serves new samples over a token-free read endpoint on the existing loopback server.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Plan decisions D1 to D5 in [resource-monitoring](../../plans/resource-monitoring.md).
- Counters must be diffed; network 32-bit counters wrap.

## Scope

**In scope:**

- `src/sampler.rs`: `Reading<T> = Value(T) | Unavailable(&'static str) | Restricted`, `Sample`, the thread, `Arc<RwLock<VecDeque<Sample>>>` of 1800 entries.
- CPU via `host_processor_info` ticks; load via `getloadavg`; memory via `host_statistics64` and `sysctl` (`hw.memsize`, `vm.swapusage`, `kern.memorystatus_vm_pressure_level`, `kern.memorystatus_level`); performance and efficiency core counts via `sysctl hw.perflevel*`; network via `sysctl NET_RT_IFLIST2` or `getifaddrs`.
- `GET /api/metrics?since=<ts>` returning samples newer than `ts`, and `sys-monitor metrics` printing one sample.
- The monitor's own CPU use recorded in each sample (`self_cpu_pct`).

**Out of scope** (name the todo that covers it):

- GPU and disk: B2.
- Per-process data: B3.
- Any chart: B4.

## Target files

| File | Change |
|---|---|
| `src/sampler.rs` | (new) |
| `src/server.rs` | route and sampler start |
| `src/main.rs` | `metrics` subcommand |
| `Cargo.toml` | `mach2` or `libc` features; check licence and version first |
| `docs/agents/layout.md` | add the file |

## Steps

- [x] Write counter-diff tests first (wraparound, backwards counter, first sample).
- [x] Implement each reading; each one that fails returns `Unavailable` with a reason, never zero.
- [x] Measure the sampler's own CPU use over 10 minutes and record it in the Implementation Record.
- [x] Add the endpoint and CLI.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| First tick (no previous counters) | Rates absent for that sample, not zero |
| 32-bit network counter wraps | `wrapping_sub` gives the right delta |
| Counter goes backwards (interface reset) | That sample is dropped |
| `sysctl` fails | `Unavailable("reason")` |
| `since` newer than every sample | Empty list |
| Buffer full | Oldest sample dropped; length stays 1800 |
| No client connected | Light metrics keep sampling; nothing heavy is added |

## Acceptance criteria

- `sys-monitor metrics` prints plausible CPU, memory, pressure and network values on this Mac.
- `/api/metrics?since=` returns only newer samples.
- Measured self CPU use is recorded and under 2% on this Mac, or the todo says why not.

## Tests

Rust unit tests for the diff helpers and the ring buffer (6 minimum); no test reads real system state except one smoke test marked macOS-only.

## Verification

```sh
make check
make test
cargo run --release -- metrics
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2

## Risks and open questions

- Whether the "used" formula (app + wired + compressed) matches Activity Monitor is unverified: record a side-by-side check. **Status:** cross-checked against `vm_stat`'s own breakdown instead (within 0.1%, see Implementation Record); the Activity Monitor GUI comparison itself was not done -- no GUI access from this session. Still open for whoever can run it.
- The endpoint is read-only but exposes activity patterns: it stays behind the Host check like the others.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no user-facing prose added; the todo itself and code comments are the only prose, not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/sampler.rs` -- `Reading<T>`, `Sample`/`CpuSample`/`MemorySample`/`NetworkSample`, the raw readers (`sysctl_into` as one shared home for every `sysctlbyname` call; `getloadavg`; `host_processor_info` for per-core ticks; `host_statistics64` for VM stats; `sysctl NET_RT_IFLIST2` for network bytes, the same source `netstat -ib` uses; `getrusage` for the sampler's own CPU), `Sampler::tick`, the 1-hour ring buffer and its background thread, `since`. Wired into `src/main.rs` (`metrics` subcommand) and `src/server.rs` (`GET /api/metrics?since=<ts>`, behind the same `Host` guard as every other route, no token since it is read-only).
- **Tests:** 40 Rust tests pass (29 before; 11 new: first-tick-no-rate, normal CPU%, a core's counters going backwards (two-core scenario so the aggregate still reports), mismatched core count, network rate, network wraparound, network counter reset, ring-buffer-drops-oldest, since-filters-correctly, an unknown sysctl name, and one macOS-only smoke test reading real system state).
- **Real run on this Mac:** `sys-monitor metrics` and the live `/api/metrics` endpoint both returned plausible values (load average, 8 cores at 6 performance/2 efficiency matching this Mac, per-core percentages, 16 GB total memory, swap, pressure level, network throughput) -- cross-checked against `vm_stat`, `netstat -ib` and `sysctl hw.perflevel*` directly.
- **Self CPU use, measured three ways:** 5 minutes isolated, 60 samples every 5 s: avg 0.99%, max 1.8%. 10 minutes while `cargo build --release` and a `git rebase` ran concurrently on the same machine, 120 samples every 5 s: avg 1.68%, one spike to 15.7%. A clean 1-minute recheck with nothing else running: avg 1.29%, max 2.2%. The spike traces to the concurrent build/rebase load on the machine during that window, not the sampler's own 2 s loop -- the isolated runs show no comparable spike. Average stays under 2% on this Mac across every run; acceptance criterion met, with the caveat recorded honestly rather than only reporting the best number.
- **`app_bytes` bug found and fixed before commit:** the first formula (`internal_page_count - purgeable_count - compressor_page_count`) computed exactly 0 on this Mac, because `compressor_page_count` exceeds `internal_page_count` here, saturating the subtraction every time -- caught by running against live data, not by the unit tests (which use synthetic counters). Replaced with `active + inactive + speculative - purgeable - external_page_count`, cross-checked against `vm_stat`'s own "Anonymous pages" line (within 0.1% on two separate runs). Whether this matches Activity Monitor's "App Memory" label exactly was not separately verified (no GUI access from this session); the risk is left open below rather than claimed as resolved.
- **Network-counter bug found and fixed before commit:** the first implementation read `getifaddrs`'s `AF_LINK` entry as `if_data64`; on this macOS version that entry is the older 32-bit `if_data`, so the read landed past the real fields -- the "byte count" it produced was actually `ifi_lastchange`, a timestamp (it decoded to 2026-11-17, days before this session). Caught by a real two-tick test showing exactly 0 bytes/sec despite live traffic, then confirmed by decoding the suspicious value as a Unix timestamp. Replaced with `sysctl(CTL_NET, PF_ROUTE, 0, 0, NET_RT_IFLIST2, 0)` reading `if_msghdr2`/`if_data64` directly, the same mechanism `netstat -ib` itself uses; verified against `netstat -ib`'s own en0 counters afterward.
- **Dependency:** `libc = "0.2.191"`, dual-licensed MIT OR Apache-2.0 (checked in its own `Cargo.toml`), already present as a transitive dependency before this todo. `mach2` was tried first and removed: it does not expose `host_processor_info`, `host_statistics64`, `vm_deallocate` or `mach_task_self`, which this todo needs directly from `libc`.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: three source comments naming todo IDs (`B1`/`B2`/`B3`/`B5`), against the repo's "no todo IDs in source files" rule -- rewritten to explain the why without the IDs; three near-identical `sysctlbyname` call sites consolidated into one `sysctl_into<T>` with a `// SAFETY:` comment, and the generic `"sysctl failed"` message replaced with one naming the sysctl per call site; missing `// SAFETY:` comments added on the two other `std::mem::zeroed()`/`getrusage` calls; a dense `zip().zip().map().or_else()` chain for `self_cpu_pct` extracted into its own named function; four repeated `CpuSample` unavailable-literals collapsed to `CpuSample::default()` (added a unit-compatible `Default` for `Reading<T>`); an unjustified one-line `thread_spawn` wrapper inlined away; a unit test added for an unknown `sysctl` name returning `Unavailable` with its reason rather than panicking. Not changed, with reasons: `Result<T, &'static str>` as the universal error type is a judgement call the reviewer flagged as mild Primitive Obsession, but it matches the todo's own `Reading<T>` spec, so kept.
- **Deviation:** none from the sketch beyond the two bug fixes above, both found by running against real data rather than assumed correct from the unit tests alone.
- **Open:** the Activity Monitor side-by-side for `app_bytes` (noted in Risks below) was not performed -- no GUI access from this session. B4 is the first consumer of this endpoint (a chart); B2 and B3 extend `Sample` with GPU/disk and per-process data respectively.
- **CI:** pending on the pull request.
