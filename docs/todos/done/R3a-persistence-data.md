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

- [x] Write the fingerprint and diff tests (written together with the code, not first).
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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** Each startup item has a schedule, a fingerprint, `removed_at`, `unreadable`, `mark_key` and `trust_note`. The fingerprint is a SHA-256 (the `sha2` crate, 0.11.1, MIT or Apache-2.0, no default features) of the plist bytes and the program's signing identity. No timestamps go in.

**Storage.** This is migration step 3, because M1 and A9 used 2. It adds five columns in one transaction: `fingerprint`, `removed_at`, `schedule`, `changed_at` and `restored_at`. A row is written only when an item is new, changed, removed or restored. A removed row is deleted 90 days after removal.

**Marks.** A startup mark is `startup:<path>#<fingerprint>`. An item is expected only while its fingerprint matches, so a marked item whose program is replaced is reported again as changed, with `was_expected` and `is_new` set. A bare path entry matches any version and is legacy.

**Changes.** The baseline date is the first scan plus the learning period. `changes` holds the last 30 days, at most 200 entries, newest first, with a `truncated` flag. An item that came back different is `changed` with `restored`. An added item that was edited says `changed_since_added`.

**Safety.** Plists are opened without blocking and must be regular files of at most 8 MiB. The same bytes are hashed and given to plutil on stdin. Scans run one at a time behind a lock, and each carries a number. A folder that could not be read never marks items removed.

**Deviation.** The endpoint returned a bare array, so it now returns `{items, changes}`. The current page reads `items` and hides removed items until R3b shows them. It sends `mark_key` and shows "Not checked" or "Program missing" instead of a dash.

**Cache merge.** The code-signing cache in `collect.rs` now keys on modified time, so one cache serves everything. A link to an ordinary plist is followed, because Homebrew installs such links.

**Tests.** The fingerprint and diff tests were written with the code, not first. They cover migration from version 2, the mark-then-change case with an unsigned program, a pipe, an oversized file, the caps, pruning, two threads, and a scan through scratch folders.

**Open.** `/writing-review` is for the owner's session. R3b must show removed items and `changes`.
