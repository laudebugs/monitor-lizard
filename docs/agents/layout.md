# Layout

| Path | What it holds |
|---|---|
| `docs/` | Everything written down; see [docs/README.md](../README.md) |
| `scripts/` | Repo tooling: todo board check, doc-index check, worktree helpers |
| `.github/` | Workflows, dependabot, PR and issue templates |
| `.agents/skills/` | Skills; `.claude/skills` symlinks here |
| `Cargo.toml`, `Cargo.lock` | The Rust crate `ownwatch` ([ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)) |
| `src/main.rs` | CLI entry: `snapshot`, `json`, `serve`, `approve`, `privilege`, `watch`, `sudo-log`, `persistence` |
| `src/collect.rs`, `src/group.rs`, `src/rules.rs`, `src/snapshot.rs`, `src/model.rs` | Process and socket collection, grouping by app, findings and the rule-based trust label (`rules::classify`), the snapshot type |
| `src/baseline.rs`, `src/history.rs` | The approved-apps baseline (`~/.ownwatch/baseline.json`) and the SQLite history (`~/.ownwatch/history.db`: first-seen hosts, startup items, sudo launches, action audit) |
| `src/privilege.rs`, `src/watcher.rs` | Authorization-log parsing and the live sudo/su launch watcher |
| `src/persistence.rs` | Read-only scan of launch agents, launch daemons and cron; `is_new` marks items first seen after the learning period |
| `src/actions.rs`, `src/server.rs` | Guarded Stop/Quit and the loopback HTTP server |
| `web/index.html` | The dashboard; the server embeds it at build time and injects the per-run token |

## One home per concept

- One home per concept: search before writing a helper or a doc.
- A new top-level folder needs a row here and, if it is a design choice, an ADR.
