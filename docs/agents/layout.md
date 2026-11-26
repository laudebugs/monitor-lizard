# Layout

| Path | What it holds |
|---|---|
| `docs/` | Everything written down; see [docs/README.md](../README.md) |
| `scripts/` | Repo tooling: todo board check, doc-index check, worktree helpers |
| `.github/` | Workflows, dependabot, PR and issue templates |
| `.agents/skills/` | Skills; `.claude/skills` symlinks here |
| `Cargo.toml`, `Cargo.lock` | The Rust crate `monitor-lizard` ([ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)) |
| `src/main.rs` | CLI entry: `snapshot`, `json`, `serve`, `approve`, `privilege`, `watch`, `sudo-log`, `persistence` |
| `src/collect.rs`, `src/group.rs`, `src/rules.rs`, `src/snapshot.rs`, `src/model.rs` | Process and socket collection, grouping by app, findings and the rule-based trust label (`rules::classify`), the snapshot type |
| `src/paths.rs` | Where state lives (`~/.monitor-lizard/`), with the one-time migration from whichever of `~/.sys-monitor/` or `~/.ownwatch/` is found |
| `src/baseline.rs`, `src/history.rs` | The baseline of expected things (`baseline.json`: typed entries `app:`, `host:`, `startup:`, `finding:<app>:<rule>#<ports>`; at most 5,000 entries, written under a lock file; an old untyped file reads as apps; a corrupt one is kept as `baseline.json.bad`; `/api/baseline`, `POST /api/approve` and `/api/unapprove` take `kind` and `key`) and the SQLite history (`history.db`: first-seen hosts, startup items, sudo launches, action audit, a `baseline_changes` audit of every mark and unmark; a `samples` table of downsampled CPU/memory history at 1 and 15 minute resolutions, opt-in via `settings.metrics_history`, never per-process; the schema is versioned with `PRAGMA user_version`, see below), both under `src/paths.rs`'s data directory |
| `src/privilege.rs`, `src/watcher.rs` | Authorization-log parsing and the live sudo/su launch watcher |
| `src/persistence.rs` | Read-only scan of launch agents, launch daemons and cron; `is_new` marks items first seen after the learning period |
| `src/actions.rs`, `src/server.rs` | Guarded Stop/Quit and the loopback HTTP server |
| `src/sampler.rs` | 2 s background sampler (CPU, memory, network, load, GPU, disk), 1-hour ring buffer, served at `/api/metrics?since=` |
| `src/iokit.rs` | Cached IOKit service handles for GPU utilization (`IOAccelerator`) and disk throughput/capacity (`IOBlockStorageDriver`, `statfs`), no process spawning |
| `src/procusage.rs` | Per-process CPU, memory and disk I/O via `proc_pid_rusage` every 5 s, keyed by (pid, start time); `ps`-derived fallback for another user's processes; `/api/processes?sort=&limit=` |
| `src/classify.rs` | `Action`, `allowed_actions(label, group)`, and the bounded `Evidence` record for one group -- what the classifier agent (A8) is allowed to suggest and see; `validate_verdict(rule_label, allowed, verdict) -> Verdict` clamps A8's raw reply to never escalate below the rule label and never suggest an action outside `allowed` |
| `src/settings.rs` | `~/.monitor-lizard/settings.json`: notification level, quiet hours, generic text, seeded |
| `src/notify.rs` | Deduplicated macOS notifications for high findings via `osascript`, off by default |
| `src/llm.rs` | One blocking call to a local llama-server with a JSON-schema response format, parsed into a `Verdict`; clamping against the rules is A10 |
| `web/index.html` | The dashboard; the server embeds it at build time and injects the per-run token |
| `tests/web/` | Node logic tests for the dashboard (`node --test`); they extract the marked `url-state` block from `web/index.html`, so keep that block free of DOM access |
| `web/fonts/` | IBM Plex Sans, bundled (three static weights, subsetted, WOFF2), embedded and served by `src/server.rs`; see `THIRD-PARTY-NOTICES.md` |

## One home per concept

- One home per concept: search before writing a helper or a doc.
- A new top-level folder needs a row here and, if it is a design choice, an ADR.

## Change the database schema with a migration

`MIGRATIONS` in `src/history.rs` is an ordered list of SQL steps, and `PRAGMA user_version` is how many have run. To add a column or table, append one step at the end; never edit or reorder a step that has shipped, because installed databases already ran the old text.

- Each step runs in its own transaction. A failing step rolls back and `open()` returns the error.
- The first time an unversioned database is migrated, `history.db.bak` is written next to it (once; never overwritten).
- Step 1 relies on `IF NOT EXISTS`, so an old database with a table that lacks a column is not repaired. Fix that with a later step, not by editing step 1.
- Opening takes an exclusive lock on `history.db.lock`, so one process at a time reads the version, makes the backup and migrates.
- A database whose version is higher than the list is refused untouched. Downgrades are not supported.
- A new table that nothing alters later can still be a `CREATE TABLE IF NOT EXISTS` inside a new step.
- Test a step on a database built at the previous version, as the `a_step_added_later_...` test does.
