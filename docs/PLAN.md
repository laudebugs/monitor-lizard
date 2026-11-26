# ownwatch plan

Goal: continuously answer "is everything running and talking on this Mac mine?"

## Prior art (researched 2026-11; unverified items marked)
| Tool | Collection | Licence | Gap vs our goal |
|---|---|---|---|
| Little Snitch 6 | Network Extension + Endpoint Security | proprietary | Firewall; no "not mine" verdict, no persistence view |
| LuLu (Objective-See) | Network Extension (unconfirmed) | GPL-3.0 | Alerts/rules only; no inventory |
| KnockKnock, BlockBlock, TaskExplorer, Netiquette, OverSight, ReiKey | persistence scan, sockets, signing | GPL-3.0 | Best coverage, but split across apps; no history or baseline |
| Santa (northpolesec) | Endpoint Security exec logging | Apache-2.0 | Exec only, no network |
| osquery | SQL tables; es_process_events needs FDA | OSS | No UI or baselining |
| Mac Monitor (Red Canary) | Endpoint Security, ~41 event types | OSS | Event firehose, no verdicts, needs entitlement |
| Aftermath (Jamf) | one-shot forensic collection | MIT | Not continuous |
Unverified: Lynis, Fleet/Kolide, Vallum, Radio Silence, Murus, commercial EDRs.

**Our niche:** a continuous, grouped inventory with a baseline ("mine") and a first-seen diff, which none of the above combine.

## Apple API reality
- Endpoint Security needs the restricted `com.apple.developer.endpoint-security.client` entitlement. Reports of 5-13 month waits, per-team applications, and denials for distribution. Dev testing means a VM with SIP/AMFI off (2019 guidance; recheck on current macOS).
- Network Extension content filter needs a paid account (not verified) and only matters if we want blocking.
- NetworkStatistics.framework (used by nettop) is private. Do not depend on it.
- Free-tier path: polling via libproc / lsof / codesign / plist scans / unified log.

## Decisions
- **Rust core**, polling-first. Crates: `libproc` (done: real image path via proc_pidpath), `rusqlite` (history), `endpoint-sec` (phase 6, if entitlement granted). Shell out to `codesign`/`spctl` for signatures (apple-codesign looks stale).
- **UI**: local web dashboard now (`web/index.html`, served on 127.0.0.1). Later a Tauri v2 menu-bar shell showing the "unknown" count that opens the same dashboard.

## Phases
1. **Inventory (done)**: ps + proc_pidpath + lsof + codesign, grouping, rules, CLI, dashboard with approve.
2. **History (done)**: SQLite first-seen remote host per app, flagged for approved apps (60s learning window, 1h flag window); background recorder every 10s.
3. **Baselines**: first-seen remote host per app; learning period; Trust / Ignore / Investigate inbox.
4. **Enrichment**: reverse DNS, ASN/owner.
5. **Persistence**: LaunchAgents/Daemons, login items, cron, profiles; diff vs baseline.
6. **Privilege events (done, v1)**: authd log parsed into sudo/admin requests with requester and parent (parent only if still alive). Next: capture parent chain in real time.
7. **Real-time (optional)**: Endpoint Security helper. Apply for the entitlement early.
8. **Menu-bar app** (Tauri v2) and notifications for high findings only.

## Risks
1. ES entitlement may never be granted to an individual.
2. Polling misses short-lived processes and connections; other users' sockets need root.
3. No public non-root per-process packet attribution.
4. "Not mine" needs a trust model (signature, path, install source) to avoid false positives.
5. Unverified: SIP-off ES workaround on current macOS; NE terms for free accounts.

## Principles
Read-only, local-only (127.0.0.1, Host and Origin checks), no telemetry, every finding explainable in one line.
