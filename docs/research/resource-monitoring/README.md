# Resource monitoring on macOS without root

As of November 2026. Gathered on 2026-11-26 by an agent that read docs and source and ran a few read-only commands (`sysctl`, `ioreg`, `vm_stat`, `netstat`, `iostat`) on one Mac running macOS 27.0.1. Items marked [U] are unverified; some points rest on the agent's background knowledge because page fetches sometimes returned no detail. This report did not go through the five-step pipeline in [../README.md](../README.md).

**Thesis:** CPU, memory, memory pressure, GPU utilization, disk and network throughput, battery and per-process CPU, memory and disk I/O are readable by a normal user. Power draw, temperatures, fans and per-process GPU are not, or are fragile, and the UI must say so instead of showing zeros.

## Per metric (no root)

| Metric | Source | Rust approach | Confidence |
|---|---|---|---|
| CPU total, per core, user/system | `host_processor_info(PROCESSOR_CPU_LOAD_INFO)` ticks, diffed | `mach2` or raw FFI, or `sysinfo` | High [U: not re-verified] |
| Performance vs efficiency cores | `sysctl hw.perflevel0.*` (performance) and `hw.perflevel1.*` (efficiency); here 6 and 2 logical CPUs. Mapping cores to clusters needs IORegistry `cluster-type` | `sysctl` plus IOKit | High for counts, medium for the mapping |
| Load average | `getloadavg()` | libc | High |
| CPU frequency | No simple API; macmon reads IORegistry `voltage-states*` plus IOReport; units differ by chip | Skip in the first slice | Low to medium |
| GPU utilization | IORegistry `IOAccelerator` > `PerformanceStatistics`: "Device Utilization %", "Renderer Utilization %", "Tiler Utilization %" (read here without root; `ioreg` took about 24 ms) | `io-kit-sys` + `core-foundation` | High |
| GPU power and frequency | IOReport (private `libIOReport.dylib`) | `macmon` crate | Medium; fragile on macOS 27 |
| Memory | `host_statistics64(HOST_VM_INFO64)`; `sysctl hw.memsize`, `vm.swapusage` (read here) | FFI, `sysinfo` for totals | High on fields; the "used" formula (app + wired + compressed) is inference |
| Memory pressure | `sysctl kern.memorystatus_vm_pressure_level` (1, 2, 4) and `kern.memorystatus_level` (percent available); read here: 2 and 47 | libc | High that it reads; one report says the level stayed "normal" during swap thrash, so show the percentage too [U] |
| Disk capacity | `statfs`/`statvfs`; "important usage" for purgeable-aware free space [U] | libc | High |
| Disk throughput and IOPS | IORegistry `IOBlockStorageDriver` > `Statistics` [U: key names from memory]; `iostat` works but blocks about 1 s | `io-kit-sys` | Medium |
| Network throughput | `sysctl NET_RT_IFLIST2` (64-bit counters) or `getifaddrs` (32-bit, wraps); `netstat -ib` took 167 ms | `sysinfo`, libc | High |
| Temperatures | `IOHIDEventSystemClient` (what `sysinfo` does) | `sysinfo`, `macmon` | Medium; sensor names change per chip |
| Fans | SMC (unofficial keys); fanless Macs report none | `macmon` | Low to medium [U] |
| Battery | `ioreg -r -c AppleSmartBattery`, `IOPSCopyPowerSourcesInfo` (read here) | `io-kit-sys` | High |
| `powermetrics` | Refuses to run without superuser (verified here) | Not usable | High |
| Per-process CPU, memory, disk I/O | `proc_pid_rusage`: `ri_user_time`, `ri_system_time` (mach time; convert), `ri_phys_footprint`, `ri_diskio_bytesread`, `ri_diskio_byteswritten` (V4 and later). Own processes only | `libproc` or FFI | High on fields; minimum macOS version not stated [U] |
| Per-process GPU time | Not in `rusage_info_v4` to `v6`; a third-party note says IORegistry client entries carry `accumulatedGPUTime` [U] | Later, if verified | Low |

**Key finding:** on macOS 27, IOReport CPU and Neural Engine power counters can stay visible but stop updating without a private entitlement; macmon 0.12.1 works around it with hidden reports and warns that zero may mean "unavailable". Treat power as best-effort.

## Crates (from crates.io on 2026-11-26)

`sysinfo` 0.40.4 (CPU, memory, disks, networks, processes, components; no GPU, IOPS, power or pressure), `macmon` 0.12.1 (power, frequency, temperature, pressure; uses private IOReport; `get_metrics` blocks for its interval), `libproc` 0.15 (in use). `mach2`, `io-kit-sys`, `core-foundation` and `battery` were not checked: verify version, date and licence before adopting.

## Shell versus system calls (measured)

`vm_stat` 3 ms, `ioreg -c IOAccelerator` 24 ms, `netstat -ib` 167 ms, `iostat -c2 -w1` about 1 s. Prefer in-process calls; shell out only as a fallback.

## Existing tools

Stats (exelban) covers CPU, GPU, memory, disk, network, battery and sensors; its README says the sensors and Bluetooth modules are the most expensive. macmon samples at 1000 ms by default. Activity Monitor, iStat Menus, btop, asitop and mactop were not researched.

## Sampler and storage (the agent's proposal, not benchmarked)

One sampler thread at 2 s; light metrics every tick, heavy ones (per-process table, battery, temperatures) every 5 to 10 s. A ring buffer of 1800 samples (one hour). Optional SQLite in WAL mode with downsampling (2 s for 1 h, 1 min for 48 h, 15 min for 30 days). Diff counters with `wrapping_sub`; drop a sample when a counter goes backwards; key per-process rates by pid and start time. The monitor must not be its own top consumer.

## Delivery to the browser

`tiny_http` 0.12 is blocking and each SSE client would hold a thread; whether a response without a length streams and flushes per event is not confirmed. Recommendation: poll `GET /api/metrics?since=<ts>` every 2 s for new points only, pause while the tab is hidden, add SSE later only if needed. Render with inline SVG paths, downsampled; charts need `role="img"`, an `aria-label` and a text summary.

## Safety and privacy

Process names and activity patterns reveal behaviour: stay on loopback, keep Host and Origin checks, do not write per-process history to disk by default. Metrics are for reading; never auto-kill from them. A pid can be recycled, so re-check pid and start time at the moment of Stop and show evidence from a fresh sample.

## Not verified

Everything marked [U]; the unresearched tools; `sysinfo` Apple Silicon temperature support; whether `tiny_http` can stream SSE.
