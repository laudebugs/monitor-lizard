# S1b: Converging storage scan

| Field | Value |
|---|---|
| ID | `S1b` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D2, D3 |
| Depends on | S1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s1b-converging-storage-scan` |
| PR title | `feat(storage): converge on a full scan across runs` |
| Size | M (200-400) |
| PR | [#92](https://github.com/laudebugs/monitor-lizard/pull/92) |

## Goal

Make the storage scan finish over several runs. Today every scan starts from zero, so on a developer home the same first locations are measured and the rest are never reached. Keep what each scan learned, visit the stale and unmeasured locations first, and merge results instead of replacing them.

## Context

- S1 measured a developer home in 20 s at 20% of one core and reached about 270 of 305 locations. The scan is partial on a normal developer home, not only on an unusual one. Without this todo the result never converges.
- The server scan needs a longer budget than the command line. The owner approved the work, not a number, so the budget is the 120 s proposed here (see Steps and plan D17).
- The data folder helpers for owner-only files (PR #41) are in `main` and this todo uses them.

## Scope

**In scope:**

- A per-location result map, `{bytes, complete, measured_at, mtime_hint}`, kept in memory and optionally saved under the data folder (owner-only).
- Visit order: never-measured and stale locations first, then by last known size, largest first.
- Reuse a location whose top-level modification time is unchanged, as a hint only. The largest locations are refreshed in turn so a change deep inside is still found.
- Merge results into the map; never replace the map with one partial scan.
- A longer budget for the server's background scan: 120 s at 20% of one core, as proposed. The command-line `storage` stays at 20 s.
- `/Applications` measured per app bundle, each bundle with its own entry cap, so the folder can exceed 200,000 entries in total.
- What the Storage page (S2a) must show for partial data, written into S2a's Context.

**Out of scope:**

- Capacity history: S4.
- Any removal: S3a and S3b.

## Target files

| File | Change |
|---|---|
| `src/storage/mod.rs` | scan order from the map; merge |
| `src/storage/results.rs` | (new) the result map and its optional file |
| `src/storage/cache.rs` | the server scan's longer budget |

## Steps

- [x] Server budget set to the proposed 120 s and recorded in the plan (D17). The owner has not chosen the number separately; it is one constant to change.
- [x] Add the result map and merge rules; test that a partial scan never lowers a complete earlier result.
- [x] Order locations: stale or never measured first, then by last known size, largest first.
- [x] Reuse an unchanged location by its top-level modification time, and refresh the largest ones round-robin.
- [x] Measure `/Applications` per bundle.
- [x] Save the map owner-only, and ignore a file that is corrupt or from a newer build.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| First run, nothing saved | Same as S1 |
| Location unchanged since last scan | Reused and marked with when it was measured |
| Location's top-level time unchanged but a deep file grew | Found when its turn in the round-robin comes |
| A partial scan of a location that was complete before | The earlier complete value is kept, marked stale |
| Saved map is corrupt | Ignored; the scan starts empty |
| Location deleted since last scan | Dropped from the map |

## Acceptance criteria

- Running the scan repeatedly on a developer home reaches every location within a few runs, and the result says `complete: true` once it has.
  - Checked on a synthetic tree of 506 locations: converged in 3 runs. Not checked on a real developer home. A location with an unreadable subfolder never reports `complete: true`, so a home that has one never does either.
- Each row says how long ago it was measured, and a reused row says so.
  - Checked in the JSON (`measured_at`, `reused`, `stale`). The page that shows it is S2a.
- The monitor stays at or below 25% of one core during a scan.
  - Synthetic numbers: the first full scan took 2.3 s at 20% of one core. Reused scans took 0.2 s at 21 to 24%, because the location search before the walks is not paced. The `node_modules` search took 0.15 s over 40 projects. Not measured on a real developer home or for a full 120 s.

## Tests

Rust tests, 10 minimum: merge rules, ordering, reuse by time hint, round-robin refresh, corrupt file, owner-only file mode.

## Verification

```sh
make check
make test
cargo run --release -- storage | head -40
```

## Docs to update

`docs/agents/layout.md`, the plan's status line, S2a's Context.

## What S2a must show for partial data

- A "Partial" banner while `complete` is false.
- "At least" before every total whose `lower_bound` is true.
- Per row, "measured N min ago", and "not yet scanned" rows for locations in `not_reached`.
- Never a final treemap, and never a "safe to remove" total, while `complete` is false.

## Risks and open questions

- A time hint can miss a change deep inside a folder. The two oldest of the eight largest unchanged locations are walked again each scan, and so is the oldest one in twenty of all the others. So a change is found within about 20 scans that are at least 10 minutes apart; until then the row keeps its old size, and a Trash preview measures the item again first.
- A transient permission error can lower a shown size until the next scan: a walk that reaches the end replaces the old value with what it could read. The Trash refuses incomplete rows anyway.
- The 20-scan bound also needs the scan to reach the location before its budget ends. Locations due for the long-tail refresh are walked right after the never-measured ones, so a scan that runs out of time still reaches them.
- Two processes saving at the same moment (the command line and the server) can lose one update; the later write wins and the next scan measures the location again.
- A longer background scan uses disk for longer. The 20% CPU limit and the low priority stay.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

### Decision

The owner said "let's do S1b", which approves the work. The server budget of 120 s at 20% of one core is this todo's proposal, not a figure the owner chose separately. It is recorded in the plan as D17 and lives in one constant, `SERVER_TIME_BUDGET` in `src/storage/cache.rs`. The command-line `storage` stays at 20 s.

### What was built

- `src/storage/results.rs`: the result map and its file, `storage-results.json` in the data folder (0600 in a 0700 folder). A corrupt, oversized, symlinked, foreign-owned, newer-version or implausible file is ignored.
- `merge`: a walk that reached the end replaces the entry, even when an unreadable subfolder keeps it incomplete. A walk cut off by time or the entry cap keeps the larger size, stays incomplete and marks an earlier complete value stale, so it never lowers it.
- Visit order: never-measured locations first, in the fixed order; then stale ones (incomplete, changed or due), largest known size first; then unchanged ones, which are reused without a walk.
- Reuse and refresh: an unchanged top-level modification time reuses the last size. Each scan walks again the two oldest of the eight largest unchanged locations and the oldest one in twenty of the rest, none more often than every 10 minutes.
- Each entry in `/Applications` is its own location with its own entry cap.
- A location that disappeared is dropped from the map. A scan that runs out of time shows the earlier size, marked stale, instead of "not reached".
- Items carry `measured_at`, `reused` and `stale`. S2a's Context says how the Storage page must show them.

### Verified and not verified

- Verified: unit tests on temporary folders under a throwaway `HOME` (order, reuse, round-robin, merge, caps, deletion, saved file mode, corrupt and newer files); `make check`, `make test`, `make check-docs`.
- Review: `/code-review` (standards and spec axes) found a window in which a reused row could be shown with an old size after the folder changed during a long scan. The hint is now checked again at the visit. It also found a cached empty map after a refused home, a duplicated unchanged test, a stray comment about approval and a deleted location shown as stale when time ran out. All are fixed with tests.
- Second review round (coordinator, same PR), findings and fixes:
  - A folder that shrank but kept one unreadable child showed its old size forever. A walk that reaches the end now replaces the size, and a cut-off bound ages out after a week.
  - A reused row kept its old size for several scans after a deep change. A Trash preview now measures its items again, and a fresh scan reuses nothing.
  - A small location's deep change was never found. Every location is now re-walked within about 20 scans.
  - The save path now writes a new temporary file, flushes the file and the folder, merges what is on disk, and leaves a newer-version file alone.
  - A loaded file is refused past 20,000 entries or a 4096-byte path, and a name that is not valid UTF-8 is listed as not reached.
  - `visit_order` and `discover` moved to `order.rs` to keep `mod.rs` under 800 lines.
- Third review round (coordinator), findings and fixes:
  - A later preview rewrote the cached size, so an earlier confirmation still matched and would have moved a folder that grew to 5 GB. The confirmation is now bound to each item's size, entry count, completeness and modification time, so a re-measure that changes anything voids earlier ones. A test covers the exact scenario.
  - A preview could spend more than its budget measuring. Measuring is capped at 15 s for all items together, the move's time budget and the confirmation's life start after it, and over the cap the preview says "preview fewer". The map lock is no longer held across walks.
  - `fresh=1` had no test; it has one now, and the flag belongs to the scan's own thread.
  - Replacing the cached rows after a re-measure is refused when a newer scan finished meanwhile.
  - Saving caps the merged map at 20,000 entries (oldest first), clamps a future time read from disk, and refuses a folder that is a link or not ours.
  - The long-tail refresh is walked before the ordinary stale locations. The owner check is tested on the load path.
- Mutation check: removing any of these now fails a test: the file size cap, the owner check, the sizes in the confirmation, the fresh-scan flag, the scan-generation check, the merged-map cap. Removing the flush from the default save still passes: a test checks the flush calls and their order, but cannot observe a real `fsync`.
- Known gaps left: a location with an unreadable subfolder never reports `complete: true` (its size is exact for what can be read and shown as "at least"); map entries for paths that were not candidates stay in the file; the command-line text output does not print the ages.
- Not verified: a scan of a real developer home, so convergence in "a few runs" and the 25% CPU ceiling at 120 s are unmeasured. The node_modules search now runs before the walks on every scan and takes up to a quarter of the budget.
