# Resource monitoring: live CPU, GPU, memory, disk and network

- **Status:** Draft
- **Date:** 2026-11-26
- **Baseline:** `main` after the scaffold adoption PR, inspected 2026-11-26
- **Asked for by:** laudebugs
- **Builds on:** [resource monitoring research](../research/resource-monitoring/README.md), [system-monitor plan](system-monitor.md), [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)
- **Todos:** [B1](../todos/ready/B1-sampler-ring-buffer-cpu-memory-network.md), [B2](../todos/ready/B2-gpu-and-disk-metrics.md), [B3](../todos/ready/B3-per-process-usage-table.md), [B4](../todos/ready/B4-performance-overview-page.md), [B5](../todos/ready/B5-cpu-gpu-detail-page.md), [B6](../todos/ready/B6-memory-detail-page.md), [B7](../todos/ready/B7-disk-network-detail-page.md), [B8](../todos/ready/B8-top-consumers-and-guarded-stop.md), [B9](../todos/ready/B9-metrics-history-sqlite.md)

## 1. Goal

Show what the machine is doing right now and over the last 5 minutes or hour (CPU, GPU, memory, disk, network) as live graphs, with the heaviest of your own processes one click from a guarded Stop. Success: the Performance page matches the approved design, works with no root and no private entitlements, and the monitor itself never becomes the top consumer.

## 2. What the system does today

As of 2026-11-26 the app collects processes and sockets by polling and shows Overview, Apps and Privilege pages. It has no resource metrics, no sampler thread and no charts. `src/actions.rs` signals own-user processes by pid only.

## 3. What others do

See the [research](../research/resource-monitoring/README.md): Stats and macmon sample once a second and mix system calls with private frameworks for power and sensors; Activity Monitor and iStat Menus were not researched.

## 4. Decisions

- **D1.** No root and no private entitlements. Anything that needs them is shown as "unavailable" with the reason, never as zero.
- **D2.** Read through system calls (`host_*`, `sysctl`, `proc_pid_rusage`, `getifaddrs`, IOKit), not by spawning commands each tick. Shelling out is a fallback only.
- **D3.** One sampler thread at 2 s feeds an in-memory ring buffer of one hour. SQLite history is optional and later ([B9](../todos/ready/B9-metrics-history-sqlite.md)). Per-process history is never written to disk by default.
- **D4.** The browser polls `GET /api/metrics?since=<ts>` every 2 s and pauses while hidden. Server-sent events only if polling proves insufficient.
- **D5.** Every tile has three states: value, unavailable (with a reason) and restricted (another user's process). Pressure is colored on memory pressure, not on how much is used.
- **D6.** Metrics never trigger an action by themselves. Stop stays user-initiated, guarded, and re-checks pid plus start time against a fresh sample.
- **D7.** The approved design is the source of truth for layout: a Performance page with Overview, CPU and GPU, Memory, and Disk and network tabs, a 5 minute or 1 hour window, Pause and a Top consumers table (design board "Performance overview, live" and its siblings in the owner's design canvas, approved 2026-11-26).

## 5. Order of work

| Todo | Title | Depends on |
|---|---|---|
| B1 | Sampler, ring buffer, CPU, memory and network | none |
| B2 | GPU and disk metrics | B1 |
| B3 | Per-process usage table | B1 |
| B4 | Performance overview page | B1 |
| B5 | CPU and GPU detail page | B2, B4 |
| B6 | Memory detail page | B4 |
| B7 | Disk and network detail page | B2, B4 |
| B8 | Top consumers and guarded Stop | B3, B4 |
| B9 | Metrics history in SQLite | B1 |

## 6. Risks and open questions

1. IOKit key names for disk statistics and per-process GPU time are unverified.
2. CPU and Neural Engine power is fragile on macOS 27 and is out of scope; the page says "unavailable".
3. `tiny_http` cannot be assumed to stream; polling avoids the question.
4. Sampling cost has not been measured; B1 records the monitor's own CPU use.
5. The UI can only be smoke-tested in a simulated DOM unless a real browser is available.
