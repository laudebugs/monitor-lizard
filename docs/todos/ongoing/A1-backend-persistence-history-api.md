# A1: Startup-item scanner, history and persistence API

| Field | Value |
|---|---|
| ID | `A1` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a1-backend-persistence-history-api` |
| PR title | `feat(api): scan startup items and expose first-seen history` |
| Size | M (200-400) |
| PR | — |

## Goal

Add a read-only scanner for launch agents, launch daemons and cron, remember when each item was first seen, and expose startup items, first-seen hosts and the action audit log over the loopback API, so the Persistence, Network and History pages have data.

## Context

- A local WIP commit `47d0b7a` on branch `feat/persistence-history-backend` already holds this work (10 tests passing at the time). Rebase it onto `main` on the new branch and finish the steps below; do not rewrite it from scratch.
- [prior-art research](../../research/prior-art/README.md): persistence locations are the classic sign of something installing itself.
- Plan decision D6 defines "new".

## Scope

**In scope:**

- `src/persistence.rs`: scan `~/Library/LaunchAgents`, `/Library/LaunchAgents`, `/Library/LaunchDaemons` via `plutil -convert json -o -`, plus `crontab -l`; check each program's signature with `collect::signing_of`.
- `src/history.rs`: tables `persistence(path PRIMARY KEY, first_seen, last_seen)`; methods `record_persistence`, `seen_rows`, `recent_actions`.
- `src/server.rs`: `GET /api/persistence` (cached, refreshed every 60 s) and `GET /api/history` (`{seen, actions}`).
- `ownwatch persistence` CLI command.

**Out of scope** (name the todo that covers it):

- The pages that consume these endpoints: A2, A4, A5.
- Login items (need `sfltool`/BTM access, not available without root): not covered by any todo yet.

## Target files

| File | Change |
|---|---|
| `src/persistence.rs` | (new) `Item`, `item_from_json`, `item_from_cron`, `take` |
| `src/history.rs` | `persistence` table, `record_persistence`, `seen_rows`, `recent_actions`, `SeenRow`, `ActionRow` |
| `src/server.rs` | routes `/api/persistence`, `/api/history`; 60 s refresh |
| `src/main.rs` | `mod persistence`, `persistence` subcommand and usage line |
| `docs/agents/layout.md` | add `src/persistence.rs` |

## Steps

- [ ] Cherry-pick or rebase `47d0b7a` onto `main` on branch `feat/a1-persistence-history-api`.
- [ ] Add a unit test in `src/history.rs` for `record_persistence`: first scan returns `earliest` equal to `now`; a second scan 120 s later with one extra path returns that path's `first_seen` as the later time and `earliest` unchanged.
- [ ] Add a test that an item present in the first scan is never `is_new` (compare `first_seen <= earliest + 60`).
- [ ] Run `./target/release/ownwatch persistence` and `curl` both endpoints against the real machine; record counts in the Implementation Record.
- [ ] Update `docs/agents/layout.md`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Unreadable or non-plist file in a LaunchAgents folder | Skipped; no panic; other items still listed |
| `KeepAlive` is a dict, `Program` missing, `ProgramArguments` present | `keep_alive = true`; program is `ProgramArguments[0]` |
| `crontab -l` exits non-zero ("no crontab") | No cron items; no error |
| `@reboot` cron line | `run_at_load = true` |
| Program path contains spaces (`/Applications/Foo Bar.app/...`) | Signature still checked; path intact |
| First scan ever | No item is `is_new` |
| Item removed, later re-added | `first_seen` is the original time |

## Acceptance criteria

- `GET /api/persistence` returns a JSON array with `first_seen` and `is_new` on every item.
- `GET /api/history` returns `{seen: [...], actions: [...]}` and the `actions` rows match what the dashboard's Stop button wrote.
- Both endpoints refuse a foreign `Host` header like the others.

## Tests

Rust unit tests in `src/persistence.rs` and `src/history.rs`, at least 4 new cases beyond the existing 4 (plist JSON, cron lines, record/first_seen, learning window). No test touches the real home directory: use a temp path via `History::at`.

## Verification

```sh
make check\nmake test\ncargo run --release -- persistence | head\ncargo run --release -- serve &\ncurl -s localhost:4747/api/persistence | head -c 300\ncurl -s localhost:4747/api/history | head -c 300
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2 (what the system does today)

## Risks and open questions

- `plutil` and `codesign` are spawned per item: first scan may take seconds with many items. Measure and, if over 3 s, move scanning off the request path (it already is cached).
- Third-party plist `Label` values are attacker-controlled strings; the UI must render them as text (A2).

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
