# R3a: Persistence data: run schedule, fingerprints and removals

| Field | Value |
|---|---|
| ID | `R3a` |
| Status | `done` |
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

- [x] Write the fingerprint and diff tests first.
- [x] Add migration 2 and the writers.
- [x] Expose the fields and the `changes` list.

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
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** Each startup item now has a schedule, a fingerprint, `removed_at` and `unreadable`. The fingerprint is a SHA-256 of the plist bytes and the program's signing identity. No timestamps go in. The hash uses the system's CommonCrypto, so there is no new dependency.

**Storage.** The todo says migration 2, but M1 and A9 already used it, so this is migration step 3. It adds five columns in one transaction: the three asked for, plus `changed_at` and `change_kind` for the change list. A row is written only when an item is new, changed, removed or restored.

**Changes.** The baseline date is the end of the learning period after the first scan. Before it, `changes` is empty. Items marked expected leave the list. A folder that could not be read never marks its items removed.

**Deviation.** `/api/persistence` returned a bare array, so an additive `changes` field was impossible. It now returns `{items, changes}`. `loadPers` in `web/index.html` reads `items` and hides removed items, so the page looks as before. `classify.rs` and `main.rs` also changed to read `items`.

**Codesign.** Results are cached per program path and modified time, so a replaced program is read again and an unchanged one is not.

**Tests.** 18 new Rust tests, 207 in all. I wrote the code and tests together, so I did not watch them fail first.

**Real run.** On a scratch home with a version 2 database, the new columns appeared and old rows survived. Editing a plist showed `changed`. Deleting one set `removed_at` and kept it in the response. A broken plist stayed listed with plutil's reason.

**Browser.** The existing Persistence page showed 33 rows in dark and light, and removed items were hidden. I did not check other widths.

**Open.** `/code-review` and `/writing-review` are for the owner's session. R3b must surface removed items and `changes`.
