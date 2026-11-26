# S1: Storage scanner and `/api/storage`

| Field | Value |
|---|---|
| ID | `S1` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D2, D3, D4 |
| Depends on | B2 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s1-storage-scanner-and-api` |
| PR title | `feat(storage): scan known locations and serve /api/storage` |
| Size | M (200-400) |
| PR | — |

## Goal

Scan a fixed list of locations on the boot volume, label each item `safe`, `review` or `yours`, and serve the result at `GET /api/storage`. The result carries category totals, items with allocated size, and the capacity split into used, purgeable and free.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md): capacity comes from `statfs`; the purgeable-aware "important usage" figure is unverified (see Steps).
- `src/persistence.rs` and its cache in `src/server.rs` are the pattern for a scan that is cached and served as JSON.
- `src/sampler.rs` `Reading<T>`: use it so unreadable values say why.

## Scope

**In scope:**

- Locations: `~/Library/Developer/Xcode/DerivedData`, `~/Library/Developer/CoreSimulator`, `~/Library/Caches/*`, `~/Library/Containers/com.docker.docker`, `~/Library/Logs`, `~/Downloads`, `~/Library/Mail`, `/Applications`, plus a bounded search for `node_modules` (depth 6 from `~/`, skipping `~/Library`).
- Local snapshots through `tmutil listlocalsnapshots /` (count and dates).
- Size is allocated bytes (`st_blocks * 512`), never follows symlinks, never crosses devices, has a total time budget of 20 s and runs at low priority.
- `GET /api/storage` returns the cached scan; `?fresh=1` starts a new one and returns the previous result with `scanning: true`.
- A `sys-monitor storage` command that prints the same JSON.

**Out of scope** (name the todo that covers it):

- Any deletion: S3a and S3b.
- Capacity history: S4.
- The page: S2a and S2b.

## Target files

| File | Change |
|---|---|
| `src/storage.rs` | (new) scanner, rules table, types |
| `src/server.rs` | route and cache for `/api/storage` |
| `src/main.rs` | `mod storage;` and the `storage` subcommand |

## Steps

- [ ] First, on a real Mac, check how to read purgeable space (`NSURLVolumeAvailableCapacityForImportantUsageKey` or `diskutil info /`) and record the answer in the todo. If neither works without root, report free space only and mark purgeable `Unavailable`.
- [ ] Write the rules table tests first: a path maps to exactly one label; nothing under `~/Documents`, `~/Desktop` or `~/Pictures` can be `safe`.
- [ ] Implement the walker with `lstat`, the time budget and a cap of 200,000 entries per location.
- [ ] Add `Reading::Unavailable(reason)` for `~/Library/Mail` and similar when permission is denied.
- [ ] Wire the cache and route; keep the scan off the request thread.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Location missing (no Docker installed) | Item omitted, not shown as 0 |
| Permission denied | Item present with `Unavailable("permission denied")` |
| Symlink loop | Not followed; counted once |
| Sparse file (`Docker.raw`) | Allocated bytes, with apparent size in a separate field |
| Time budget hit | Partial result with `complete: false` and the locations not reached |
| Two scans requested at once | One runs, the second returns the cached result |
| `tmutil` missing or fails | Snapshots `Unavailable` with the reason |

## Acceptance criteria

- `sys-monitor storage` prints valid JSON in under 20 s on a Mac with a 100 GB developer folder.
- No item under a user-content folder is ever labelled `safe`.
- The monitor's own CPU stays under 25% of one core during a scan.

## Tests

Rust tests, 10 minimum: rules table, allocated versus apparent size on a temp sparse file, symlink loop, budget cutoff, denied directory (a temp dir with mode 000).

## Verification

```sh
make check
make test
cargo run --release -- storage | head -40
```

## Docs to update

`docs/agents/layout.md` (new module), the plan's status line.

## Risks and open questions

- Snapshot size is probably not readable without root; the row then shows a count only.
- `node_modules` search can be slow on large home folders; the budget and entry cap bound it.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
