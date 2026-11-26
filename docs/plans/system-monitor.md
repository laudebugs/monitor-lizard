# system-monitor: is everything on this Mac mine?

- **Status:** In progress
- **Date:** 2026-11-26
- **Baseline:** `main` at `6ecc2f1`, inspected 2026-11-26
- **Asked for by:** laudebugs
- **Builds on:** [prior art](../research/prior-art/README.md), [local classifier](../research/local-classifier/README.md), [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)
- **Todos:** [A1](../todos/ready/A1-backend-persistence-history-api.md), [A2](../todos/ready/A2-persistence-page.md), [A3](../todos/ready/A3-listeners-page.md), [A4](../todos/ready/A4-network-page.md), [A5](../todos/ready/A5-history-page.md), [A6](../todos/ready/A6-rule-based-labels.md), [A11](../todos/ready/A11-rename-to-system-monitor.md). Agent: [A7](../todos/ready/A7-agent-allowed-actions-and-group-query.md), [A8](../todos/ready/A8-agent-structured-call.md), [A10](../todos/ready/A10-agent-verdict-and-validator.md), [A12](../todos/ready/A12-model-install.md), [A13](../todos/ready/A13-agent-supervisor-and-fallback.md), [A15](../todos/ready/A15-agent-ui-shadow-mode.md), [A16](../todos/ready/A16-agent-eval-corpus-and-runner.md). [A14](../todos/ready/A14-notifications-for-high-findings.md). Resource monitoring is its own plan: [resource-monitoring](resource-monitoring.md).

## 1. Goal

Continuously answer one question for one person: is every process and connection on this Mac mine? Success: the Overview lists only what needs a decision, each item can be resolved in one click (approve, stop, mute), and nothing leaves the machine.

## 2. What the system does today

As of 2026-11-26, on `main`:

- `src/collect.rs`, `src/group.rs`, `src/snapshot.rs`: polls `ps`, `lsof` and `libproc`, verifies signatures with `codesign`, and groups processes and sockets by app bundle, toolchain or macOS system.
- `src/rules.rs`: findings for exposed listeners, unsigned binaries with sockets, unreviewed apps and an open debug port.
- `src/history.rs`: SQLite first-seen remote host per app; flags new hosts for approved apps after a 60 s learning window.
- `src/privilege.rs`, `src/watcher.rs`: authorization-log parsing and a 20 ms process-table watcher that records sudo/su launches with their parent chain.
- `src/actions.rs`, `src/server.rs`: guarded Stop/Quit (own-user processes only, per-run token, audit log) on a loopback server.
- `web/index.html`: the dashboard with Overview, Apps, Privilege, dark/light/system theme, custom dropdowns and paginated tables. Network, Listeners, Persistence and History are placeholders.
- An uncommitted-to-main backend for startup items and history queries exists as a local WIP commit (`47d0b7a` on `feat/persistence-history-backend`); [A1](../todos/ready/A1-backend-persistence-history-api.md) lands it.

## 3. What others do

See [prior art](../research/prior-art/README.md): the Objective-See tools cover persistence and sockets separately, Little Snitch and LuLu are firewalls, Santa and Mac Monitor need Endpoint Security. None keeps a baseline of what is yours.

## 4. Decisions

- **D1.** Rust core that polls, no entitlements ([ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)). Endpoint Security stays optional and late.
- **D2.** The UI is a static page on loopback, no remote assets.
- **D3.** Writes are guarded: per-run token, loopback Host and Origin, own-user processes only, audited.
- **D4.** Rules decide the label; a model, if added, only explains and ranks suggested actions, never downgrades a rule's "suspicious", and never acts. See [local classifier](../research/local-classifier/README.md).
- **D5.** State lives in `~/.ownwatch/` (baseline JSON, history SQLite) until [A11](../todos/ready/A11-rename-to-system-monitor.md) renames it with a migration.
- **D7.** The classifier is a bounded call from Rust to a local llama-server; buildanagent is used for evals only, not at runtime: [ADR 0003](../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) (accepted 2026-11-26), [research](../research/buildanagent-adoption/README.md).
- **D6.** "New" means first seen after a learning period (60 s after the very first scan of that kind), flagged for a bounded window (1 h for hosts, 7 days for startup items), so the first run is not a wall of alerts.

## 5. Order of work

| Todo | Title | Depends on |
|---|---|---|
| A1 | Startup-item scanner, history and persistence API | none |
| A2 | Persistence page | A1 |
| A3 | Listeners page | none |
| A4 | Network page | A1 |
| A5 | History page | A1 |
| A6 | Rule-based labels | none |
| A7 | Allowed actions per label and read-only group query | A6 |
| A8 | Structured call to a local llama-server | A7 |
| A10 | Clamp the verdict to the rules | A8 |
| A12 | Download and verify the model | A6 |
| A13 | Supervise llama-server, rules-only fallback | A10, A12 |
| A15 | Rationale and actions in the UI, shadow mode first | A13 |
| A16 | Eval corpus and pass^k runner | A10 |
| A14 | Notifications for high-severity findings | none |
| A11 | Rename to system-monitor | none; do last, it touches every file |

Not yet written (IDs A9, A17 to A19 are free): the feedback baseline ("mark as expected"), the menu-bar app, enrichment (reverse DNS and ASN: reverse DNS tells a resolver which IPs you look up, so it needs an opt-in), and optional Endpoint Security.

## 6. Risks and open questions

1. Polling misses short-lived processes and connections; other users' sockets need root.
2. "Not mine" needs a trust model (signature, path, install source) to avoid false positives; A6 is the first step.
3. The UI has only been smoke-tested in a simulated DOM; layout checks are pending a real browser.
4. Branch protection and pinned actions are not set up (see [ci-and-release.md](../agents/ci-and-release.md)).
5. Open: the final product name (the crate is `ownwatch`, the repository `system-monitor`).
