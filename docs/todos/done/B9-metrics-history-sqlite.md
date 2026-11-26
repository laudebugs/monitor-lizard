# B9: Metrics history in SQLite (optional)

| Field | Value |
|---|---|
| ID | `B9` |
| Status | `done` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b9-metrics-history-sqlite` |
| PR title | `feat(metrics): persist downsampled metrics history` |
| Size | M (200-400) |
| PR | — |

## Goal

Optionally keep downsampled system metrics (never per-process) in SQLite so the 1 hour window can grow to days: 2 s for 1 hour, 1 minute for 48 hours, 15 minutes for 30 days, averages plus maxima for CPU and memory.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Plan decision D3: per-process history is never written to disk by default.

## Scope

**In scope:**

- `samples(ts, res, metric, v)` with `PRIMARY KEY(res, metric, ts)`, WAL mode, batched writes every 30 s, roll-ups and pruning.

**Out of scope** (name the todo that covers it):

- Per-process history: not planned.

## Target files

| File | Change |
|---|---|
| `src/history.rs` | tables, writer, roll-up, prune |
| `src/server.rs` | `/api/metrics?res=` |

## Steps

- [x] Write roll-up and prune tests first.
- [x] Implement behind a setting that defaults to off.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Process killed mid-batch | At most 30 s lost; database intact (WAL) |
| Roll-up across a gap | Gap stays a gap, no interpolation |
| Setting off | Nothing written |

## Acceptance criteria

- With history on, a restart keeps the last day of charts.

## Tests

Rust tests (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- README (the data folder holds more now)

## Risks and open questions

- Disk use: measure growth over a day and record it.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/history.rs` -- a `samples(res, metric, ts, v)` table, `PRIMARY KEY(res, metric, ts) WITHOUT ROWID`, with WAL mode turned on at connection open. Only two resolutions are ever persisted: `res='1m'` (rolled up directly from the in-memory 2s ring every 30s) and `res='15m'` (cascaded from the `1m` rows, not from raw). The existing 1-hour, 2-second in-memory ring (from B1) is unaffected and untouched by this todo -- it already covers the "2 s for 1 hour" tier and was never written to disk before this todo or after it. `rollup_buckets` is the pure core: given points and a bucket width, it returns one (avg, max) row per bucket that is both new (not already rolled up) and fully elapsed, skipping entirely a bucket with no points (a gap in the source stays a gap). `rollup_1m`/`rollup_15m` resume from whatever is already in the table (`MAX(ts)`, `Option<u64>` to distinguish "nothing yet" from a real bucket at ts=0) rather than a separate watermark, so they are safe to call repeatedly, including right after a restart. `prune_metrics` deletes rows past each tier's own retention (48h, 30d). `metrics_tick` is the per-cycle orchestration, pulled out of the actual background thread (`start_metrics_writer`) so it is directly testable without a thread or a real 30s sleep. `src/settings.rs` gained `metrics_history: bool` (default false) and a 5th `update`/`update_at` parameter, all call sites updated. `src/server.rs` gained `/api/metrics?res=1m|15m` (falls back to the existing in-memory `/api/metrics?since=` behavior for `res=raw` or no `res` at all) and spawns the writer thread alongside the sampler/procusage threads it already starts.
- **Tests:** 125 total (116 before; +8 in `history::tests`, +1 in `settings::tests`), covering: WAL mode is genuinely active; a gap in the source produces no interpolated row; bucket averaging and maxima are correct; a still-filling bucket is never rolled up early and an already-rolled-up one isn't redone; `rollup_1m` is idempotent across repeated calls with the same input; `rollup_15m` genuinely reads from the `1m` tier, not raw points; `prune_metrics` deletes exactly what is past retention and nothing else; `metrics_tick` touches neither the ring nor the database when disabled; the `metrics_history` setting defaults off, can be turned on and off, and rejects a bad value.
- **Real run:** built the release binary and ran it with an isolated `HOME`, turned `metrics_history` on through the real `/api/settings` endpoint, and let it run long enough for two real 1-minute buckets to roll up. `GET /api/metrics?res=1m` returned real, plausible rows (this Mac was genuinely busy: `cpu_avg` ~80-97%, `cpu_max` 100%, `mem_avg`/`mem_max` ~13.9-14.5 GB); `sqlite3`'s own `PRAGMA journal_mode` on the real file confirmed `wal`, and the raw `samples` table rows matched the API's pivoted JSON exactly.
- **Disk use, measured (not estimated):** filled a scratch copy of the real schema to full retention on both tiers (2880 one-minute buckets + 2880 fifteen-minute buckets, 4 metric rows each = 23,048 rows) and checked the actual resulting file size: 812 KB before `VACUUM`, 753 KB after. Recorded in the README rather than left as an arithmetic guess.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards review found no hard violations (checked specifically: no AGENTS.md rule 9 todo-ID comments; the `Reading<T>` extraction in `metrics_tick` correctly skips rather than defaults to 0 for an unavailable CPU or memory reading; every `update_at` call site updated for the new 5th parameter) and one real judgement call: the new tests using `tmp_history()` never cleaned up their temp directories afterward, unlike every other test in this file -- fixed by having `tmp_history` return the directory alongside the `History` and adding `std::fs::remove_dir_all` to all six. Spec review confirmed every edge case and acceptance criterion was met, found no scope creep, and flagged that the disk-use risk item was only an estimate rather than the measurement the todo actually asked for -- fixed as described above. It also asked for an explicit verdict on not persisting a raw tier to disk: this is a deliberate, sound reading of "keep **downsampled** system metrics" in the Goal (the in-scope line only names the rollup/prune table, never a raw tier; the acceptance criterion, "a restart keeps the last day of charts," only needs the 1m/15m tiers, which do survive a restart; and a persisted raw tier would roughly 30x the write volume for data the in-memory ring can already serve) -- recorded explicitly here per that suggestion, rather than left implicit.
- **Deviation:** none from the sketch beyond the fixes above and the no-raw-tier design decision explained above.
- **Open:** none beyond B5/B6/B7/B8 remaining as separate todos.
- **CI:** pending on the pull request.
