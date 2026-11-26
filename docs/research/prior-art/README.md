# Prior art for a local macOS process and network monitor

As of November 2026. Gathered on 2026-11-26 by web research; nothing here ran code. Claims taken from search summaries are secondary. This report did not go through the five-step pipeline in [../README.md](../README.md): there is no fact-check log yet, and the items under "Not verified" are unchecked.

**Thesis:** no existing tool combines a continuous, grouped inventory of processes and sockets with a baseline of what is yours and a first-seen diff. That is our niche.

## Existing tools

| Tool | Collection | Licence | Gap against our goal |
|---|---|---|---|
| Little Snitch 6 | Network Extension plus Endpoint Security | Proprietary | A firewall: no "not mine" verdict, no persistence view |
| LuLu (Objective-See) | Network Extension (not confirmed from its page) | GPL-3.0 | Alerts and rules only; no inventory |
| KnockKnock, BlockBlock, TaskExplorer, Netiquette, OverSight, ReiKey (Objective-See) | Persistence scans, sockets, signing | Mostly GPL-3.0 | The best coverage, split across apps; no history or baseline |
| Santa (northpolesec) | Endpoint Security exec logging | Apache-2.0 | Exec only, no network data |
| osquery | SQL tables; `es_process_events` needs Full Disk Access | Open source | No UI, no baselining |
| Mac Monitor (Red Canary) | Endpoint Security, about 41 event types | Open source | An event firehose with no verdicts; needs the entitlement |
| Aftermath (Jamf) | One-shot forensic collection | MIT | Not continuous |

**Not verified:** Lynis, Fleet/Kolide, Vallum, Radio Silence, Murus and the commercial EDRs.

## Apple API reality

- **Endpoint Security** needs the restricted `com.apple.developer.endpoint-security.client` entitlement. Developers report waits of 5 to 13 months, per-team applications, and denials for distribution. Testing without it means a VM with SIP and AMFI off, per 2019 guidance. Recheck on current macOS: not verified.
- **Network Extension** content filtering needs a paid developer account (not verified) and only matters if we want blocking.
- **NetworkStatistics.framework**, which `nettop` uses, is private. Do not depend on it.
- **The free path** is polling: libproc, `lsof`, `codesign`, plist scans, the unified log.

## Rust crates (versions as of 2026-11-26, from crates.io)

`libproc` 0.15 (used), `rusqlite` 0.43 (used), `sysinfo` 0.40 (processes only, no sockets), `netstat2` 0.11 (last release 2025-08), `endpoint-sec` 0.6 (needs the entitlement), `apple-codesign` 0.29 (last update 2024-11, so we shell out to `codesign`), `security-framework` 3.7 (not checked for static-code APIs).

## Detection ideas worth borrowing

Persistence locations (LaunchAgents, daemons, login items, cron, profiles), a first-seen remote host per app, unsigned or ad-hoc signed binaries with sockets, listeners on all interfaces, odd parent-child pairs, DNS capture. Against alert fatigue: trust Apple and notarized vendors by default, aggregate by app, use a learning period, keep a "mine" allowlist. These are general knowledge, not checked against sources.
