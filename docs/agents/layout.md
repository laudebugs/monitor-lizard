# Layout

| Path | What it holds |
|---|---|
| `docs/` | Everything written down; see [docs/README.md](../README.md) |
| `scripts/` | Repo tooling: todo board check, doc-index check, worktree helpers |
| `.github/` | Workflows, dependabot, PR and issue templates |
| `.agents/skills/` | Skills; `.claude/skills` symlinks here |
| `Cargo.toml`, `Cargo.lock` | The Rust crate `sys-monitor` ([ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)) |
| `src/main.rs` | CLI entry: `snapshot`, `json`, `serve`, `approve`, `privilege`, `watch`, `sudo-log`, `persistence` |
| `src/collect.rs`, `src/group.rs`, `src/rules.rs`, `src/snapshot.rs`, `src/model.rs` | Process and socket collection, grouping by app, findings and the rule-based trust label (`rules::classify`), the snapshot type |
| `src/paths.rs` | Where state lives (`~/.sys-monitor/`), with the one-time migration from `~/.ownwatch/` |
| `src/baseline.rs`, `src/history.rs` | The approved-apps baseline (`baseline.json`) and the SQLite history (`history.db`: first-seen hosts, startup items, sudo launches, action audit), both under `src/paths.rs`'s data directory |
| `src/privilege.rs`, `src/watcher.rs` | Authorization-log parsing and the live sudo/su launch watcher |
| `src/persistence.rs` | Read-only scan of launch agents, launch daemons and cron; `is_new` marks items first seen after the learning period |
| `src/actions.rs`, `src/server.rs` | Guarded Stop/Quit and the loopback HTTP server |
| `src/sampler.rs` | 2 s background sampler (CPU, memory, network, load, GPU, disk), 1-hour ring buffer, served at `/api/metrics?since=` |
| `src/iokit.rs` | Cached IOKit service handles for GPU utilization (`IOAccelerator`) and disk throughput/capacity (`IOBlockStorageDriver`, `statfs`), no process spawning |
| `src/procusage.rs` | Per-process CPU, memory and disk I/O via `proc_pid_rusage` every 5 s, keyed by (pid, start time); `ps`-derived fallback for another user's processes; `/api/processes?sort=&limit=` |
| `src/classify.rs` | `Action`, `allowed_actions(label, group)`, and the bounded `Evidence` record for one group -- what the classifier agent (A8) is allowed to suggest and see; `validate_verdict(rule_label, allowed, verdict) -> Verdict` clamps A8's raw reply to never escalate below the rule label and never suggest an action outside `allowed` |
| `src/settings.rs` | `~/.sys-monitor/settings.json`: notification level, quiet hours, generic text, seeded |
| `src/notify.rs` | Deduplicated macOS notifications for high findings via `osascript`, off by default |
| `src/llm.rs` | One blocking call to a local llama-server with a JSON-schema response format, parsed into a `Verdict`; clamping against the rules is A10 |
| `web/index.html` | The dashboard; the server embeds it at build time and injects the per-run token |
| `web/fonts/` | IBM Plex Sans, bundled (three static weights, subsetted, WOFF2), embedded and served by `src/server.rs`; see `THIRD-PARTY-NOTICES.md` |

## One home per concept

- One home per concept: search before writing a helper or a doc.
- A new top-level folder needs a row here and, if it is a design choice, an ADR.
