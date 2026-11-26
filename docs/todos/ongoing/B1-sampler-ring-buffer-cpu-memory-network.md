# B1: Sampler, ring buffer and the CPU, memory and network metrics

| Field | Value |
|---|---|
| ID | `B1` |
| Status | `ongoing` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b1-sampler-ring-buffer-cpu-memory-network` |
| PR title | `feat(metrics): add the sampler thread and CPU, memory and network metrics` |
| Size | M (200-400) |
| PR | — |

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

- [ ] Write counter-diff tests first (wraparound, backwards counter, first sample).
- [ ] Implement each reading; each one that fails returns `Unavailable` with a reason, never zero.
- [ ] Measure the sampler's own CPU use over 10 minutes and record it in the Implementation Record.
- [ ] Add the endpoint and CLI.

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

- Whether the "used" formula (app + wired + compressed) matches Activity Monitor is unverified: record a side-by-side check.
- The endpoint is read-only but exposes activity patterns: it stays behind the Host check like the others.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
