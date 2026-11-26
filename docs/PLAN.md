# ownwatch plan

Goal: continuously answer "is everything running and talking on this Mac mine?"

## Landscape (what exists, what we borrow)
- **Little Snitch / LuLu (Objective-See)**: per-app outbound firewall via Network Extension. Borrow: per-app + per-host rules. Gap: no inventory view, no baseline diff.
- **KnockKnock / BlockBlock (Objective-See)**: persistence enumeration and monitoring (LaunchAgents, login items, cron). Borrow: persistence checklist.
- **osquery**: SQL over processes, sockets, launchd, signatures. Option for the collector later.
- **Apple Endpoint Security + Network Extension**: real-time exec/file/socket events. Needs entitlements and a signed Swift system extension.

## Model
process -> group (app bundle / toolchain / macOS system) -> sockets (listen, outbound, local) -> rules -> findings.
A group is "mine" once approved in `~/.ownwatch/baseline.json`; anything new or changed is surfaced.

## Phases
1. **Inventory (done in scaffold)**: ps + lsof + codesign, grouping, rules, CLI.
2. **Dashboard**: grouped view, per-group drilldown, approve button, findings feed.
3. **History**: SQLite snapshots every 30s; diff "new since yesterday"; first-seen remote hosts per app.
4. **Enrichment**: reverse DNS, ASN/owner, geo; flag hosts never seen before for an app.
5. **Persistence**: LaunchAgents/Daemons, login items, cron, profiles, kexts; diff against baseline.
6. **Real-time (optional)**: Swift Endpoint Security helper streaming exec/connect events over a local socket.
7. **Alerts**: macOS notifications for high findings only.

## Rules in place
exposed-listener, unsigned-network, unreviewed, debug-port. Next: new-remote-for-app, root-process-network, unsigned-persistence.

## Principles
Read-only, local-only (binds 127.0.0.1, rejects cross-origin writes), no telemetry, findings must be explainable in one line.
