# R3a: Persistence data: run schedule, fingerprints and removals

| Field | Value |
|---|---|
| ID | `R3a` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11 |
| Depends on | M1 |
| ADRs | None |
| Branch | `feat/r3a-persistence-data` |
| PR title | `feat(persistence): record run schedule, fingerprints and removals` |
| Size | M (200-400) |
| PR | — |

## Goal

Collect what the redesigned Persistence page needs and the scanner does not provide today: when each item runs, a fingerprint to detect changes, and the time an item disappeared.

## Context

- A2 (done) shows label, kind, scope, program, trust and date added. `History` keeps `persistence(path, first_seen, last_seen)`.
- M1 adds migrations; this todo is its first user.

## Scope

**In scope:**

- Read `RunAtLoad`, `KeepAlive` and `StartInterval` from each plist through the existing `plutil` path.
- A fingerprint per item: a hash of the plist bytes and the program's code-signing identity.
- Migration 2 adds `fingerprint`, `removed_at` and `schedule` columns to `persistence`.
- Write a row only when its fingerprint changes; set `removed_at` when a path is no longer found and clear it if it returns.
- `GET /api/persistence` returns these fields and a `changes` list (added, changed, removed since the baseline date).

**Out of scope** (name the todo that covers it):

- The page: R3b. Disabling an item: not planned.

## Target files

| File | Change |
|---|---|
| `src/persistence.rs` | plist keys, fingerprint |
| `src/history.rs` | migration 2, writers, `changes()` |
| `src/server.rs` | add fields to `/api/persistence` |

## Steps

- [ ] Write the fingerprint and diff tests first.
- [ ] Add migration 2 and the writers.
- [ ] Expose the fields and the `changes` list.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Plist cannot be parsed | Item listed with "unreadable" and the reason; not dropped |
| Program path missing | Trust shows "program missing" |
| Item removed then restored | One row; `removed_at` cleared; the change list shows the restore |
| First run (no baseline) | Everything counts as baseline; `changes` is empty |
| Unchanged item on every scan | No write |

## Acceptance criteria

- Editing a plist's program path shows the item under `changes` after the next scan.
- Deleting a plist sets `removed_at`, and the item still appears in the response.

## Tests

Rust tests, 8 minimum.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/` notes on the history schema; plan status.

## Risks and open questions

- Hashing the program's signature needs `codesign`; scans reuse the cached result per path and modified time.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
