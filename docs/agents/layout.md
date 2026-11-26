# Layout

| Path | What it holds |
|---|---|
| `docs/` | Everything written down; see [docs/README.md](../README.md) |
| `scripts/` | Repo tooling: todo board check, doc-index check, worktree helpers |
| `.github/` | Workflows, dependabot, PR and issue templates |
| `.agents/skills/` | Skills; `.claude/skills` symlinks here |
| `Cargo.toml`, `Cargo.lock` | The Rust crate `monitor-lizard` ([ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)) |
| `src/main.rs` | CLI entry: `snapshot`, `json`, `serve`, `approve`, `privilege`, `watch`, `sudo-log`, `persistence`, `storage` |
| `src/collect.rs`, `src/group.rs`, `src/rules.rs`, `src/snapshot.rs`, `src/model.rs` | Process and socket collection, grouping by app, findings and the rule-based trust label (`rules::classify`), the snapshot type |
| `src/paths.rs` | Where state lives (`~/.monitor-lizard/`), with the one-time migration from whichever of `~/.sys-monitor/` or `~/.ownwatch/` is found, and the startup pass that makes the folder 0700 and its files 0600 |
| `src/baseline.rs`, `src/history.rs` | The baseline of expected things (`baseline.json`: typed entries `app:`, `host:`, `startup:`, `finding:<app>:<rule>#<ports>`; at most 5,000 entries, written under a lock file; an old untyped file reads as apps; a corrupt one is kept as `baseline.json.bad`; `/api/baseline`, `POST /api/approve` and `/api/unapprove` take `kind` and `key`) and the SQLite history (`history.db`: first-seen hosts, startup items, sudo launches, action audit, a `baseline_changes` audit of every mark and unmark; a `samples` table of downsampled CPU/memory history at 1 and 15 minute resolutions, opt-in via `settings.metrics_history`, never per-process; the schema is versioned with `PRAGMA user_version`, see below), both under `src/paths.rs`'s data directory |
| `src/privilege.rs`, `src/watcher.rs` | Authorization-log parsing and the live sudo/su launch watcher |
| `src/persistence.rs` | Read-only scan of launch agents, launch daemons and cron. Each item has a schedule, a fingerprint (SHA-256 via the `sha2` crate of the plist bytes and the program's signing identity; `codesign` results are cached per path and modified time in `collect.rs`), and, when gone, `removed_at`. A plist that cannot be parsed, is not a regular file or is over 8 MiB stays listed with `unreadable`. The plist is read once and the same bytes are hashed and given to `plutil` on stdin. A relative program path has no signing result and carries a `trust_note`. `take()` returns `{items, changes, truncated, scanned_at, scan_id}`, which `/api/persistence` serves; scans run one at a time behind a lock. `is_new` marks items first seen after the learning period, and also a marked item that has changed |
| `src/storage/mod.rs` | The scan: a fixed list of folders in a set order, allocated bytes, a 20 s budget with a quarter of the time left per location, coverage and category totals that say when they are a lower bound; `monitor-lizard storage` prints it. Read-only |
| `src/storage/rules.rs`, `src/storage/walk.rs` | `label_for` / `label_item`: one `safe`, `review` or `yours` label per path, `safe` only on an allowlist and never under user content; the `lstat` walker with entry cap, device check and pacing to about 20% of one core; the `node_modules` search and the idle test |
| `src/storage/volume.rs`, `src/storage/cache.rs` | Capacity split into used, purgeable and free (CoreFoundation, no root) and local snapshots from `tmutil`; the cached result served at `/api/storage` (`?fresh=1` starts a scan at most once a minute). `/api/volume` serves the capacity alone, read on each request with no scan, for the Disk and network tab |
| `src/netif.rs` | How many network interfaces are up, running and addressed (`getifaddrs`, loopback left out); the sampler stores it as `network.interfaces_up` so the page can tell no connection from an idle one |
| `src/actions.rs`, `src/server.rs` | Guarded Stop/Quit and the loopback HTTP server |
| `src/sampler.rs` | 2 s background sampler (CPU, memory, network, load, GPU, disk), 1-hour ring buffer, served at `/api/metrics?since=` |
| `src/iokit.rs` | Cached IOKit service handles for GPU utilization (`IOAccelerator`) and disk throughput/capacity (`IOBlockStorageDriver`, `statfs`), no process spawning |
| `src/procusage.rs` | Per-process CPU, memory and disk I/O via `proc_pid_rusage` every 5 s, keyed by (pid, start time); `ps`-derived fallback for another user's processes; `/api/processes?sort=&limit=` |
| `src/classify.rs` | `Action`, `allowed_actions(label, group)`, and the bounded `Evidence` record for one group -- what the classifier agent (A8) is allowed to suggest and see; `validate_verdict(rule_label, allowed, verdict) -> Verdict` clamps A8's raw reply to never escalate below the rule label and never suggest an action outside `allowed` |
| `src/settings.rs` | `~/.monitor-lizard/settings.json`: notification level, quiet hours, generic text, seeded |
| `src/notify.rs` | Deduplicated macOS notifications for high findings via `osascript`, off by default |
| `src/llm.rs` | One blocking call to a local llama-server with a JSON-schema response format, parsed into a `Verdict`; clamping against the rules is A10 |
| `web/index.html` | The dashboard; the server embeds it at build time and injects the per-run token. Every page renderer returns `pageShell({ title, subtitle, filters, body, inspector })`; the breadcrumb and search sit in a fixed top bar so the search box keeps focus while a poll redraws the page |
| `tests/web/` | Node logic tests for the dashboard (`node --test`); they extract the marked `url-state`, `export`, `guide-search`, `guide-copy`, `page-shell`, `view-items` and `disk-net` blocks (capacity bar sizing, rate formatting, no-connection state) from `web/index.html`, so keep `url-state`, `export`, `page-shell` and `guide-search` free of DOM and storage access. `tests/fixtures/` holds the Guide table and the search results that `cargo test` and the web tests both check |
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
- Step 3 adds `fingerprint`, `removed_at`, `schedule`, `changed_at` and `restored_at` to `persistence` with `ALTER TABLE ADD COLUMN`. A row from before it has a NULL fingerprint, which the next scan fills in without reporting a change. A scan writes a row only when an item is new, changed, removed or restored.
- Startup items: `changes` covers the last 30 days and at most 200 entries, newest first, and `truncated` says when entries were left out. A removed row is deleted 90 days after `removed_at`. The baseline date is the first scan plus the learning period.
- A startup mark is `startup:<plist path>#<fingerprint>`, made by the server from the item's `mark_key`. An item is expected only while its fingerprint equals the marked one. A bare `startup:<path>` entry (legacy, hand-written) matches any version.
