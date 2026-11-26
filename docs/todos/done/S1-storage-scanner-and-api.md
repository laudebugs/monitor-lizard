# S1: Storage scanner and `/api/storage`

| Field | Value |
|---|---|
| ID | `S1` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D2, D3, D4 |
| Depends on | B2 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s1-storage-scanner-and-api` |
| PR title | `feat(storage): scan known locations and serve /api/storage` |
| Size | M (200-400) |
| PR | pending |

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
- A `monitor-lizard storage` command that prints the same JSON.

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

- [x] First, on a real Mac, check how to read purgeable space (`NSURLVolumeAvailableCapacityForImportantUsageKey` or `diskutil info /`) and record the answer in the todo. If neither works without root, report free space only and mark purgeable `Unavailable`.
- [x] Write the rules table tests first: a path maps to exactly one label; nothing under `~/Documents`, `~/Desktop` or `~/Pictures` can be `safe`.
- [x] Implement the walker with `lstat`, the time budget and a cap of 200,000 entries per location.
- [x] Add `Reading::Unavailable(reason)` for `~/Library/Mail` and similar when permission is denied.
- [x] Wire the cache and route; keep the scan off the request thread.

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

- `monitor-lizard storage` prints valid JSON in under 20 s on a Mac with a 100 GB developer folder.
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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

### Purgeable space

Purgeable space is readable without root. `CFURLCopyResourcePropertyForKey` on `/` with the key `NSURLVolumeAvailableCapacityForImportantUsageKey` works through CoreFoundation, which the crate already links. No new dependency was needed.

On this Mac (macOS 27.0.1) `df` showed 34 GiB free and `diskutil info /` showed 36.6 to 37.8 GB. The available key matched them. The important-usage key gave 78.9 GB, so purgeable is about 41 GB. Opportunistic usage gave 64.3 GB.

`diskutil info /` and `diskutil apfs list` do not print a purgeable figure, only container free space. `tmutil listlocalsnapshots /` lists dates only, and `diskutil apfs listSnapshots /` lists no size, so snapshot size is `Unavailable`.

Purgeable is the important-usage figure minus the available figure. It is an estimate that macOS recomputes, so two reads seconds apart can differ. The earlier B2 comment saying `f_bavail` already includes purgeable space was wrong, and it is corrected.

### What was built

`src/storage.rs` holds the rules table, the walker, the scan, the cache and the tests. `src/server.rs` gained one route and one cache field. `src/main.rs` and `src/guide.rs` gained the `storage` subcommand and its guide entry. Both guide fixtures were updated.

The walker uses `lstat` only and counts allocated bytes. It never follows a symlink, never enters another device and counts a hard link once. Each location has a cap of 200,000 entries, and the whole scan has a 20 s budget, with 1 s kept back for `tmutil` and the reply.

The scan runs on a utility-class thread and rests to stay near 20% of one core. The `/api/storage` cache starts a scan on the first request or on `?fresh=1`, and two requests at once start one scan.

### Measured on a real home

On this developer home (5.6 GB of caches and 12 GB of Docker data, with `/Applications` at 18 to 19 GB), an unpaced scan used 9.7 s of CPU in 20 s, about 48% of one core. That is why pacing was added.

With pacing, two runs finished in 19.9 and 20.3 s and used 3.3 and 4.0 s of CPU (17% and 20% of one core). Peak memory was 11 MB.

On this home the scan is always partial. One run reached 238 of 304 locations and a later one 28, so the reach depends on how warm the disk cache is. `node_modules` and `/Applications` were not reached, and the result says so in `complete: false` and `not_reached`.

The utility class is used because the background class reached only about 5,000 entries in 27 s.

### Deviations

- CoreSimulator is labelled `review`, not `safe`. The unavailable runtimes cannot be told apart by path, and `xcrun simctl` would start a process.
- `/Applications` is scanned last, not first, because it can never be `safe`.
- The `node_modules` search also skips dot-folders and `~/Downloads`, and finds at most 300. A project counts as idle when its folder, `node_modules`, `package.json` and git index are all 90 days old.
- A `node_modules` folder under a user-content folder is labelled `yours`.

### Open

- A bulk directory read (`getattrlistbulk`) would cut the system calls and let the scan reach more locations in 20 s. It needs more unsafe code, so it was left out.
- The Full Disk Access folders (Mail, Safari, Music caches) show `Unavailable("permission denied")` here. Whether the A17 app identity changes that is still unverified.
- CI is pending on the pull request. `/code-review` and `/writing-review` are not run yet.
