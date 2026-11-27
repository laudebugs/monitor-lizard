# S4: Capacity history and forecast

| Field | Value |
|---|---|
| ID | `S4` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D6 |
| Depends on | S1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s4-capacity-history-and-forecast` |
| PR title | `feat(storage): keep daily capacity history and forecast when the disk fills` |
| Size | M (200-400) |
| PR | — |

## Goal

Store one capacity sample a day and show "grew 14.2 GB this week" and "full in about 20 days" on the Storage page.

## Context

- B9's metrics history is optional and off by default. Capacity history holds nothing about apps, so this todo uses its own small table and does not depend on that setting.

## Scope

**In scope:**

- A daily write of used, purgeable and free bytes, taken from the S1 capacity reading, into a new `capacity_daily` table created with `CREATE TABLE IF NOT EXISTS`. No migration is needed.
- `GET /api/storage/history` returning the series.
- Forecast: least-squares line over the last 14 days of used (including purgeable); report days until used reaches capacity and a range from the line's standard error.
- Page: the status strip fields and a small chart with the forecast as a dashed line.

**Out of scope** (name the todo that covers it):

- Per-folder history: not planned.

## Target files

| File | Change |
|---|---|
| `src/history.rs` | capacity rows, prune |
| `src/storage.rs` | forecast function |
| `src/server.rs` | route |
| `web/index.html` | strip fields, chart |

## Steps

- [x] Tests for the forecast first.
- [x] Write the daily sample at first scan after local midnight; skip if one exists for the day.
- [x] Chart as inline SVG with `role="img"` and an `aria-label` that states the forecast.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Fewer than 7 days of data | "Collecting history", no forecast |
| Flat or shrinking usage | "Not growing", no days figure |
| Large one-day drop (a big clean-up) | Fit uses days after the drop; "large" is a fall of more than 5% of the disk (decision recorded in the plan, D6) |
| Database not writable | Page still shows current capacity; the forecast says history is unavailable |
| Clock moves backwards | Sample ignored |

## Acceptance criteria

- After 8 days of samples the strip shows a forecast with a range.
- No row older than 90 days is kept.

## Tests

Rust tests, 8 minimum: fit on synthetic series, each edge case above.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status.

## Risks and open questions

- A straight line over 14 days misleads after a sudden change in habits; the range and the "last 14 days" label say so.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

### What was built

- `src/storage/forecast.rs` (new): `Day`, `forecast()`, `week_change()`, `reply()`, 14 tests.
- `src/history.rs`: `capacity_daily(day, ts, used, purgeable, free)`, `record_capacity`, `capacity_series`, `local_day`, `capacity_tick`, `start_capacity_writer`, 7 tests.
- `GET /api/storage/history` in `src/server.rs`: read-only, behind the Host check, no token.
- `web/index.html`: a Growth block in the Storage page's startup-disk card (this week, disk full, the basis of the line) with an inline SVG chart, and 7 Node tests in `tests/web/storage-page.test.mjs`.

### Review fixes (PR #57)

- `GET /api/storage/history` is now read-only in fact: a read-only SQLite connection (`read_capacity_series`), no table creation, no migration, no write lock. The table is made when the database is opened. A missing file or table is an empty series; an unreadable database is `available: false`. Tests: the route leaves the database file byte-identical, a reader is not blocked by a writer mid-transaction, the reader creates nothing.
- A row dated more than a day ahead is deleted, so one wrong clock cannot stop sampling for good; a row slightly ahead skips the sample. Pruning runs on every call. The writer logs a skipped sample once a day, an unreadable capacity once a day, and every open or store failure.
- The writer keeps one connection, drops it after a failure, and catches a panic in a tick.
- The reply says how old the last sample is and `stale` after 2 days. The page then says "Forecast out of date: last sample <date>", still gives "about N days" and the range as counted from then, shows a "Last sample" row, and no longer calls an old weekly change "this week".
- The growing note says "over the last 14 days" and that the range is about one standard error, roughly two chances in three.
- Tests split and fixture-tied; `local_utc_offset` has a range test; the unsafe block stays in `history.rs` with a note on why it is not in `iokit.rs`.
- Not tested at server level: a full `serve_on` round trip. It starts the sampler, watcher and writers against the real data folder, so the route's reply function and the Host check are tested as functions instead.
- A clock far ahead can prune up to 30 days of valid old rows early (the prune cutoff follows the clock). Rows older than 60 days are the only ones at risk.

### Deviations

- The forecast is in `src/storage/forecast.rs`, not `src/storage.rs`: `storage` is a folder.
- The daily write is not tied to a scan. A thread checks every 10 minutes and stores the first reading after local midnight, using the same capacity reading the scan uses. A history that only grew when someone opened the Storage page would have gaps.
- The table is created on first use with `IF NOT EXISTS` and no migration step, as the todo says, so `user_version` is unchanged.
- A "large one-day drop" is a fall of more than 5% of the disk in used space (purgeable included) between two consecutive samples. The todo gives no figure; this is a judgement call.
- A purgeable share that cannot be read is stored as 0, because `used` then already includes it. A sample is skipped when used or free cannot be read.
- An unopenable database gives a 200 with `available: false`, not a 500, so the page can keep the capacity bar and say why there is no forecast.
- The forecast counts days from the last sample, not from now, and fits the fitted value at the last sample, not the last raw one.

### Checked

- `make check`, `make test` (322 Rust tests, 198 Node tests), `make check-docs`.
- A scratch `HOME` with 8 synthetic earlier days plus the live capacity sample: the page showed "Grew 1.1 GB this week", "Full in about 12 days. Range 11 to 12 days", a solid line, a dashed forecast reaching the top rule, and an `aria-label` that states the forecast. Dark and light themes both looked right in Chrome. The real `~/.monitor-lizard` was not touched.

### Not verified

- Looked at in Chrome (scratch home, both themes, no resize): Collecting history, Not growing, History unavailable (the database path replaced by a folder) and the out-of-date forecast. A "Full in ... more than 2 years" result from tiny noisy growth was also seen and reads correctly.
- The safe subtotal beside Growth could not be shown: it needs a home under /Users/<name>, which cannot be created without administrator rights, and the real home must not be scanned.
- After the rebase onto S2b, the clean-ups list, Inside System Data and the Growth block rendered together in both themes. The safe subtotal showed only "Not shown until the scan is complete": the scratch home is not under /Users, so no item was labelled safe and the scan was partial. A non-zero safe subtotal beside the Growth block was not seen.
- The "Collecting history", "Not growing", "History unavailable" and failed-read states were checked only in Node tests, not in a browser.
- A database that is not writable was tested by a path that cannot be created, not by a read-only file on a real disk.
- The write after local midnight, a real clock moving backwards and a daylight-saving change were tested with injected times, not by waiting or changing the system clock.
- Narrow window widths, forced colors and reduced motion were not checked; the window was not resized.
- Screen reader output was not heard; only the label text was read.
- `/code-review` and `/writing-review` have not been run on this PR. CI is pending.
- The Tauri shell was not run.
