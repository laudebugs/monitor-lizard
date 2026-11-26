# B2: GPU and disk metrics

| Field | Value |
|---|---|
| ID | `B2` |
| Status | `ready` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b2-gpu-and-disk-metrics` |
| PR title | `feat(metrics): add GPU utilization and disk metrics to the sampler` |
| Size | M (200-400) |
| PR | — |

## Goal

Add GPU device, renderer and tiler utilization and disk throughput, operations and capacity to the sampler.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- GPU: IORegistry `IOAccelerator` > `PerformanceStatistics` was readable without root and fast (about 24 ms through `ioreg`); use the registry directly.
- Disk statistics key names in `IOBlockStorageDriver` are unverified: confirm them first.

## Scope

**In scope:**

- GPU percentages via IOKit (`io-kit-sys`, `core-foundation`); disk read and write bytes and operations per second via `IOBlockStorageDriver` statistics (diffed); capacity via `statfs`, including purgeable space if a supported call exists.

**Out of scope** (name the todo that covers it):

- Power, temperatures and fans: not planned (need root or private access).
- Per-process GPU: not measured.

## Target files

| File | Change |
|---|---|
| `src/sampler.rs` | `gpu`, `disk` fields |
| `src/iokit.rs` | (new) registry helpers with cached service handles |
| `Cargo.toml` | `io-kit-sys`, `core-foundation`: check versions and licences |

## Steps

- [ ] Confirm the registry keys on this Mac with `ioreg` and record them.
- [ ] Implement with cached handles; no process spawning in the loop.
- [ ] Return `Unavailable` when a key is missing (VMs, other chips).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No `IOAccelerator` entry | `gpu` is `Unavailable("no GPU registry entry")` |
| Disk statistics key absent | `Unavailable`, capacity still reported |
| Counter reset | Sample dropped |
| Multiple disks | Aggregate for the boot volume; list others later |

## Acceptance criteria

- GPU and disk values appear in `ownwatch metrics` and `/api/metrics`.
- The loop spawns no child processes (assert with a test double or by inspection).

## Tests

Rust tests for key parsing from canned registry dictionaries (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- Unsafe FFI: keep it in `src/iokit.rs` with each unsafe block commented with its invariant.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
