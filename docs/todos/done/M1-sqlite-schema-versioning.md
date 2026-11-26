# M1: SQLite schema versioning

| Field | Value |
|---|---|
| ID | `M1` |
| Status | `ongoing` |
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

- [ ] Write the tests first: new database, old database without `user_version`, database newer than the code.
- [ ] Implement `migrate()` and call it from `open()`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Fresh database | All migrations run; `user_version` equals the count |
| Existing database from before this todo (`user_version` 0) | Migration 1 is a no-op on existing tables; version set to 1; backup made |
| Database newer than the code | Open refuses with a clear message and changes nothing |
| A migration fails midway | Rolled back; version unchanged |
| Two processes open at once | One migrates; the other waits on the lock |

## Acceptance criteria

- Opening a pre-existing `~/.sys-monitor/history.sqlite` keeps every row and sets `user_version` to 1.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
