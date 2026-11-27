# S1b: Converging storage scan

| Field | Value |
|---|---|
| ID | `S1b` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D2, D3 |
| Depends on | S1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s1b-converging-storage-scan` |
| PR title | `feat(storage): converge on a full scan across runs` |
| Size | M (200-400) |
| PR | — |

## Goal

Make the storage scan finish over several runs. Today every scan starts from zero, so on a developer home the same first locations are measured and the rest are never reached. Keep what each scan learned, visit the stale and unmeasured locations first, and merge results instead of replacing them.

## Context

- S1 measured a developer home in 20 s at 20% of one core and reached about 270 of 305 locations. The scan is partial on a normal developer home, not only on an unusual one. Without this todo the result never converges.
- A second owner decision is needed: a longer budget for the server scan (see Steps).
- The data folder helpers for owner-only files are in PR #41; use them once they are merged.

## Scope

**In scope:**

- A per-location result map, `{bytes, complete, measured_at, mtime_hint}`, kept in memory and optionally saved under the data folder (owner-only).
- Visit order: never-measured and stale locations first, then by last known size, largest first.
- Reuse a location whose top-level modification time is unchanged, as a hint only. The largest locations are refreshed in turn so a change deep inside is still found.
- Merge results into the map; never replace the map with one partial scan.
- A longer budget for the server's background scan: propose 120 s at 20% of one core. This is an explicit owner decision. The command-line `storage` stays at 20 s.
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

- [ ] Decide the server budget with the owner (120 s proposed) and record it in the plan.
- [ ] Add the result map and merge rules; test that a partial scan never lowers a complete earlier result.
- [ ] Order locations: stale or never measured first, then by last known size, largest first.
- [ ] Reuse an unchanged location by its top-level modification time, and refresh the largest ones round-robin.
- [ ] Measure `/Applications` per bundle.
- [ ] Save the map owner-only, and ignore a file that is corrupt or from a newer build.

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
- Each row says how long ago it was measured, and a reused row says so.
- The monitor stays at or below 25% of one core during a scan.

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

- A time hint can miss a change deep inside a folder; the round-robin bounds how long it stays missed.
- A longer background scan uses disk for longer. The 20% CPU limit and the low priority stay.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
