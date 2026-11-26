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

- Locations: `~/Library/Developer/Xcode/DerivedData`, `~/Library/Developer/CoreSimulator`, `~/Library/Caches/*`, `~/Library/Containers/com.docker.docker`, `~/Library/Logs`, `~/Downloads`, `~/Library/Mail`, `/Applications`, plus a bounded search for `node_modules` in the usual project folders (the home folder and its direct children, and folders such as `~/Developer` or `~/work` to depth 5; never `~/Library` or the user-content folders).
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
| `src/storage/` | (new) `mod.rs` (scan), `rules.rs` (labels), `walk.rs` (walker and project search), `volume.rs` (capacity, `tmutil`), `cache.rs` (cache and rate limit) |
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
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (not run: the owner's session does it)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26, updated after review.

### Purgeable space

Purgeable space is readable without root. `CFURLCopyResourcePropertiesForKeys` on `/`, with the keys `NSURLVolumeAvailableCapacityKey` and `NSURLVolumeAvailableCapacityForImportantUsageKey`, works through the CoreFoundation wrappers the crate already uses. No new dependency and no hand-written extern.

On this Mac (macOS 27.0.1) `df` showed 34 GiB free and `diskutil info /` 36.6 to 37.8 GB. The available key matched them. The important-usage key gave 78.9 GB, so purgeable is about 41 GB. Opportunistic usage gave 64.3 GB.

`diskutil info /` and `diskutil apfs list` print no purgeable figure. `tmutil listlocalsnapshots /` lists dates only, and `diskutil apfs listSnapshots /` lists no size, so snapshot size is `Unavailable`. Purgeable is an estimate that macOS recomputes, so two reads can differ.

### What was built

`src/storage/` holds `mod.rs` (the scan), `rules.rs` (labels), `walk.rs` (walker, pacing, project search), `volume.rs` (capacity and `tmutil`) and `cache.rs` (the cache and its rate limit). `src/server.rs` gained one route and one cache field. The `storage` subcommand and its guide entry are in `src/main.rs` and `src/guide.rs`.

The walker uses `lstat` only and counts allocated bytes. It never follows a symlink, never enters another device and counts a hard link once. Each location has a cap of 200,000 entries and at most a quarter of the time left. The whole command has a 20 s budget with `tmutil` running beside the walk.

### Labels

A wrong `safe` tells a user to delete something, so the order of the rules is the safety argument. Path components are compared in lowercase, a path with `..`, a relative path or a component that is not UTF-8 is `review`, and an empty home refuses `safe`. User content, Mail and every `review` rule come before any `safe` rule.

`safe` is limited to Xcode DerivedData, the Homebrew, pip, Yarn and node-gyp caches, and idle `node_modules`. A generic app cache is `review`, because some apps keep offline data there. A `safe` label is kept only if the path, with symlinks resolved, gets the same label.

A project is idle only when it has a lockfile beside `package.json`, the nearest git index up to the home folder is 90 days old, and no top-level entry or `node_modules` is newer. A monorepo package has no lockfile of its own, so it is never idle. This is deliberate.

### Search for node_modules

The search enters only the home folder and its direct children, plus `~/Developer`, `~/Projects`, `~/Code`, `~/src`, `~/dev`, `~/work`, `~/git`, `~/repos`, `~/workspace` and folders with those names one level down. It never enters user-content folders or `~/Library`, so it cannot raise a folder-access prompt. It finds at most 300.

### Order and budget

Order: DerivedData, the Docker folder, app caches largest first by a one-level size, CoreSimulator, Logs, `node_modules`, Downloads, Mail, `/Applications`. The first ones are large and can be acted on; the last ones cannot be `safe`.

The JSON says how complete it is: `coverage`, `progress`, `lower_bound` and `unmeasured` per category, a stable `id` per item and `notes`. APFS clones are counted once per copy, as `du` does, and `notes` says so.

A request starts a scan at most once a minute. A request inside that window gets the cached result and `next_scan_allowed_at`.

### Measured on a real home

Two runs of `monitor-lizard storage` took 19.4 s of wall time, `tmutil` included, and used 3.9 s of CPU, about 20% of one core. Peak memory was 11 MB. Before pacing the same scan used about 48% of a core.

On a normal developer home the scan is `complete: false`, and it will stay so until S1b lets it finish over several runs. Protected folders under `Library/Caches` add to this. The two runs measured 269 and 277 of 305 and 309 locations, about 36 and 39 GB. Downloads, Mail, `/Applications` and the `node_modules` search were not reached.

Acceptance holds as written: valid JSON in under 20 s, no `safe` item under user content, and CPU under 25% of one core. Convergence over several runs is a separate todo.

### Deviations

- The `node_modules` search covers the home folder and its direct children, plus named project folders to depth 5, not depth 6 from `~/`.
- A subfolder that cannot be read makes its location, its category and the whole scan incomplete. This is common under `Library/Caches` on macOS.
- CoreSimulator is `review`: unavailable runtimes cannot be told apart by path.
- The scan is partial within 20 s, so `complete` and `not_reached` are part of the result.
- The utility class is used, not the background class, which reached about 5,000 entries in 27 s.
- The review fixes landed as one restructure, not a split commit followed by fixes, because the split and the new types touched the same lines.

### Size

The PR is far above the 200 to 400 line target: about 1,470 lines in `src/storage/`, of which about 600 are tests, plus docs. Review asked for the rules, coverage fields and cache tests, and each is part of the safety case.

### Open

- Convergence across runs is S1b.
- Mail, Safari and Music caches show `Unavailable("permission denied")` without Full Disk Access.
- `getattrlistbulk` would cut system calls and reach more locations. It needs more unsafe code.
- CI is pending. `/writing-review` is not run.
