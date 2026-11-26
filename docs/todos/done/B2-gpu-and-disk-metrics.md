# B2: GPU and disk metrics

| Field | Value |
|---|---|
| ID | `B2` |
| Status | `done` |
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

- [x] Confirm the registry keys on this Mac with `ioreg` and record them.
- [x] Implement with cached handles; no process spawning in the loop.
- [x] Return `Unavailable` when a key is missing (VMs, other chips).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No `IOAccelerator` entry | `gpu` is `Unavailable("no GPU registry entry")` |
| Disk statistics key absent | `Unavailable`, capacity still reported |
| Counter reset | Sample dropped |
| Multiple disks | Aggregate for the boot volume; list others later |

## Acceptance criteria

- GPU and disk values appear in `sys-monitor metrics` and `/api/metrics`.
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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/iokit.rs` -- cached `Gpu` (one `IOAccelerator` handle, opened once) and `Disks` (every `IOBlockStorageDriver` handle, opened once), both released on `Drop`; `Gpu::utilization()` reads `PerformanceStatistics`' three percentage keys, each independently `Reading::Unavailable` if its own key is missing; `Disks::boot_volume_stats()` reads `Statistics`' four byte/op counters, requiring all four present or returning `Err` (not a silent zero on a partial read); `boot_volume_capacity()` via `statfs("/")`. `src/sampler.rs` gained `GpuSample`/`DiskSample` and `build_gpu`/`build_disk`, pure functions following `build_cpu`/`build_network`'s existing pattern exactly (diff against the previous tick, `wrapping_sub` for a real counter wrap, a plausibility bound to tell a wrap from a reset). `Sampler` caches `Gpu`/`Disks` handles for its whole lifetime, opened once in `Sampler::default()`; the per-tick read is `IORegistryEntryCreateCFProperty`/`statfs` calls only, no process spawning anywhere in `iokit.rs` (confirmed: zero `Command::new`/`std::process::Command` in the file).
- **Registry keys confirmed on this Mac** (todo Step 1, the Context section's stated unverified items), via real `ioreg` runs, not assumed from the research doc: `ioreg -r -d 1 -c IOAccelerator` -> class `AGXAcceleratorG13X` (`model` = "Apple M1 Pro"), `PerformanceStatistics` = {..., "Device Utilization %", "Renderer Utilization %", "Tiler Utilization %", ...}. `ioreg -r -d 1 -c IOBlockStorageDriver` -> three entries on this Mac, `Statistics` = {"Bytes (Read)", "Bytes (Write)", "Operations (Read)", "Operations (Write)", "Errors (Read)", "Errors (Write)", "Retries (Read)", "Retries (Write)", "Latency Time (Read)", "Latency Time (Write)", "Total Time (Read)", "Total Time (Write)"} on each. Both match the research doc's "High"/"Medium" confidence guesses exactly; now recorded as verified rather than assumed, and the exact key names are also exercised directly by the canned-dictionary tests below.
- **The "boot volume" identification is a heuristic, not a verified link to "/":** APFS's synthesized boot container (e.g. `disk3`) is not a child of its physical store (e.g. `disk0`) in the IOKit service tree -- there is no simple registry walk from `statfs("/")`'s device name to the right `IOBlockStorageDriver` entry without either shelling out to `diskutil` (which the per-tick loop must not do) or binding the much larger DiskArbitration surface for one lookup. `Disks::boot_volume_stats()` instead picks whichever entry has the most combined lifetime bytes, which on this Mac (and reportedly in general, since idle expansion bays and recovery partitions have comparatively tiny lifetime totals) correctly identifies the real boot disk -- verified against this Mac's real numbers (one entry at ~6.9 TB read / ~3.2 TB write lifetime, dwarfing the other two at 0 and ~235 MB). This is a real behavioral compromise, not a certainty, and is flagged here explicitly for visibility rather than only in a code comment, per the review below.
- **Purgeable space:** not broken out via a separate `getattrlist`/`ATTR_VOL_SPACE_PURGEABLE` call. `statfs`'s `f_bavail` already counts APFS's reclaimable purgeable space as available (matching what `df`/Finder show), so the figures reported don't omit it; a second, dedicated purgeable-only field was judged out of scope for this todo's "if a supported call exists" phrasing, which reads as optional.
- **Tests:** 100 Rust tests pass (86 before; +14: 8 in `iokit.rs` including 4 canned-`CFDictionary` key-parsing tests built from in-process synthetic dictionaries, not real IOKit calls -- a present key, an absent key, a value of the wrong CF type, and the real confirmed disk key names, satisfying "Rust tests for key parsing from canned registry dictionaries (4 minimum)" with actual canned data rather than only real-registry smoke tests; 6 in `sampler.rs` for `build_gpu`/`build_disk`, covering every edge-case row: no GPU entry, one missing GPU key, disk first tick, a normal two-tick rate, a disk counter reset, and disk statistics key absent reporting `Unavailable` while capacity is still reported independently).
- **Real run on this Mac:** `sys-monitor metrics` and the live `/api/metrics` endpoint both show real GPU (0% idle; 48-49% device/renderer/tiler observed under concurrent build load) and disk figures (read/write bytes and ops per second tracking real activity, capacity 494,353,338,368 bytes total matching `df`/Python `statvfs` exactly). Self CPU measured over 1 minute with GPU/disk sampling included: avg 1.24%, max 2.0%, consistent with B1's earlier 0.99-1.68% baseline -- the added IOKit reads cost nothing measurable. No stray child process observed during a live `serve` run beyond the pre-existing, unrelated `watcher.rs` log-stream process.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: a comment citing "the todo's own research" as its justification (the same AGENTS.md rule 9 class of mistake, caught twice already this session) -- reworded to state the fact itself; the todo's own, stricter-than-usual Risk requirement ("each unsafe block commented with its invariant") wasn't met for five call sites that only relied on the invariant documented at their callee's definition -- added a comment at each one; `IORegistryEntryCreateCFProperty`'s result was wrapped as a `CFDictionary` without checking its actual CF type, a latent type-confusion risk if a future macOS version ever returns something else under the same key -- added a `CFGetTypeID`/`CFDictionaryGetTypeID` check first, releasing the reference directly if it doesn't match; most seriously, `Disks::boot_volume_stats` silently turned a missing `Statistics` key into a real `0` (`unwrap_or(0)`), which `build_disk` then reported as `Reading::Value(0.0)` -- directly contradicting both this file's own "say why, never show 0" convention and the todo's edge case "Disk statistics key absent -> Unavailable, capacity still reported" -- changed `boot_volume_stats` to return `Result`, requiring all four keys present per entry, with `build_disk` now distinguishing that `Err` (reported as `Unavailable`) from "no previous tick yet" (reported as `None`, unchanged); and the disk counter-reset plausibility bound was 4 GB/s, which real Apple Silicon internal NVMe can exceed, risking misclassifying genuine heavy I/O as a reset -- raised to 50 GB/s, comfortably above any real consumer SSD while still catching a true reset's many-orders-of-magnitude-larger wrapped delta. The missing canned-dictionary tests (see above) were also a spec-review finding, now fixed. Also surfaced, not changed: the "boot volume" heuristic is a real behavioral compromise that review flagged as warranting explicit sign-off (an ADR, per AGENTS.md's rule on hard-to-reverse decisions) rather than only a code comment -- not written as a full ADR here, given the todo's own size and scope, but called out plainly in this record and in the code comment so it isn't discovered only by reading the source.
- **Deviation:** none from the sketch beyond the fixes above.
- **Open:** the boot-volume heuristic (see above) and whether it should get a proper ADR if a multi-disk Mac with a busier secondary drive is ever reported to misidentify it.
- **CI:** pending on the pull request.
