# M1: SQLite schema versioning

| Field | Value |
|---|---|
| ID | `M1` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), risk 7 |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/m1-sqlite-schema-versioning` |
| PR title | `feat(history): version the database schema and run ordered migrations` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Add a migration mechanism to `src/history.rs`, so later todos can change existing tables. Today the schema is `CREATE TABLE IF NOT EXISTS` only, which cannot add a column to a table that already exists.

## Context

- `History::open` in `src/history.rs` runs one `execute_batch` that creates `seen`, `sudo_events`, `persistence`, `actions` and `notified`.
- R3a adds columns to `persistence`. New tables in other todos need no migration.

## Scope

**In scope:**

- A `MIGRATIONS` list of SQL steps and `PRAGMA user_version` as the applied count.
- Migration 1 is today's schema, written so it is safe on a database that already has those tables.
- Each migration runs in a transaction; a failure leaves the database at the previous version and reports the error.
- Before the first migration on an existing file, copy it to `history.sqlite.bak` once.

**Out of scope** (name the todo that covers it):

- The schema changes themselves: R3a and others.

## Target files

| File | Change |
|---|---|
| `src/history.rs` | `MIGRATIONS`, `migrate()`, call from `open()` |

## Steps

- [x] Write the tests first: new database, old database without `user_version`, database newer than the code (written after the first draft of `migrate()`, see Implementation Record).
- [x] Implement `migrate()` and call it from `open()`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Fresh database | All migrations run; `user_version` equals the count |
| Existing database from before this todo (`user_version` 0) | Migration 1 is a no-op on existing tables; version set to 1; backup made |
| Database newer than the code | Open refuses with a clear message and changes nothing |
| A migration fails midway | Rolled back; version unchanged |
| Two processes open at once | One migrates; the other waits on the lock |

## Acceptance criteria

- Opening a pre-existing `~/.monitor-lizard/history.sqlite` keeps every row and sets `user_version` to 1.

## Tests

Rust tests, 6 minimum, using temp databases.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/` note on how to add a migration; plan status.

## Risks and open questions

- Downgrades are not supported; the "newer than the code" case makes that explicit.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (pending, owner runs it)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** `src/history.rs` has a `MIGRATIONS` list and `migrate()`, called from `History::at` before the WAL switch. `PRAGMA user_version` counts applied steps. Step 1 is the full current schema, including the `samples` table from the metrics-history todo, written with `IF NOT EXISTS`.

**What was already there.** The metrics-history todo added WAL mode and the `samples` table but no version mechanism, so this todo built the only one. Step 1 covers `samples` too.

**Behaviour.** Each step runs in an immediate transaction and re-checks the version after taking the lock, so a second opener waits and then skips the step. A failing step rolls back. A database with a higher version is refused with a message, before any change, including the journal mode. A busy timeout of 10 seconds backs the wait.

**Backup.** An unversioned file that already holds tables is copied once to `history.db.bak` with `VACUUM INTO`, which captures data still in the write-ahead log. The copy goes to a unique name and is linked into place, so two openers cannot write the same file.

**Tests.** 7 new tests in `history::tests` cover the todo's five edge cases, a later step on an old database, and a four-thread race. The suite passes: 145 run, 2 ignored, after merging the rename.

**Real run.** An online copy of the real `history.db` in a scratch home, opened by the debug binary: version 0 became 1, `seen` (1,024 rows) and `persistence` (37 rows) were unchanged, and the backup held the same rows at version 0. The real data folder was not touched.

**Deviations.** The todo says `history.sqlite` and `history.sqlite.bak`; the file is `history.db`, so the backup is `history.db.bak`. I drafted `migrate()` before writing the tests, so the tests were not seen failing first. The "two processes" case is tested with threads, not separate processes.

**Open.** `/code-review` and `/writing-review` are for the owner's session. A failed backup link reports `InvalidPath`, which is a loose error type. Downgrades are unsupported by design.
