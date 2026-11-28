# S5: Per-app disk writes (opt-in)

| Field | Value |
|---|---|
| ID | `S5` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D7 |
| Depends on | S2a, B3, B9 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s5-per-app-disk-writes` |
| PR title | `feat(metrics): opt-in hourly disk writes per app` |
| Size | M (200-400) |
| PR | — |

## Goal

Show which apps wrote the most to disk in the last 24 hours, with an hourly trace per app and the lifetime SSD writes figure where readable. Off by default.

## Context

- B3 reads per-process disk bytes from `proc_pid_rusage` for your own processes.
- Plan D7 and the resource-monitoring rule: per-process history is not written by default.
- Lifetime writes (NVMe SMART "data units written") may need access this app does not have; unverified.

## Scope

**In scope:**

- A setting `disk_writes_history` (off by default) in `src/settings.rs`, shown in the Storage page with a plain explanation and an "Erase history" button.
- When on: sum per-app-group write deltas each hour, keep 7 days, never store file names or pids.
- `GET /api/storage/writes` and the page panel from design D.
- Lifetime writes: try to read it without root; if not possible, hide the figure and say why.

**Out of scope** (name the todo that covers it):

- Per-file write detail: not planned.

## Target files

| File | Change |
|---|---|
| `src/settings.rs` | the setting |
| `src/history.rs` | hourly table, prune, erase |
| `src/sampler.rs` | hourly roll-up from B3 data |
| `src/server.rs` | route, erase route (token-guarded) |
| `web/index.html` | panel, setting control |

## Steps

- [x] Tests for roll-up and counter resets first.
- [x] Change `settings::update` from four fixed parameters to a struct, so a new setting does not need another parameter.
- [x] Key per-process counters by pid and start time so a reused pid is not mixed in.
- [x] Add the erase route behind the same Origin and token checks as other writes.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Setting off | Nothing stored; panel shows how to enable |
| Process exits mid-hour | Its bytes so far are counted once |
| Counter goes backwards | Sample dropped |
| Another user's process | Not counted; panel footnote says own processes only |
| Erase pressed | All rows removed; panel returns to empty |

## Acceptance criteria

- With the setting on for 24 hours the panel shows non-zero writes for at least the busiest app.
- With the setting off, `~/.monitor-lizard/` holds no per-app rows.

## Tests

Rust tests, 8 minimum.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; the privacy paragraph in `README.md` and `SECURITY.md`.

## Risks and open questions

- App names with hourly counts reveal habits. The data stays local, is off by default and can be erased.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Stays in `ongoing/`.** The work is built, but `make check-docs` refuses a done todo whose dependency is unfinished, and S2a is still in `ongoing/`. S3b waits on S2a the same way. Move this file to `done/` once S2a is done.
- **Built:** `src/history/disk_writes.rs` holds the `disk_writes_hourly(hour, app, bytes)` table, made by `CREATE TABLE IF NOT EXISTS` in `History::at` as the plan's risk item 7 says new tables are. `WriteRollup` sums write-counter growth per app group and hour, keyed by (pid, start time). The hook that `procusage::start` now takes runs it after every 5 s pass, writes pending bytes once a minute and prunes rows older than 7 days, at most hourly while the setting is off too.
- **Built, around the table:** `Tracker::write_counters` (`src/procusage.rs`) gives each own process's lifetime `ri_diskio_byteswritten`, named by `group::group_of(exe).label`, never the path. `settings::Update` replaces the five fixed `update` parameters. `GET /api/storage/writes` (read-only connection, token required because the rows reveal habits) and `POST /api/storage/writes/erase` are in `src/server.rs`. The Storage page panel is `stWrites` in `web/page-storage.js`, with its pure logic in the `storage-writes` block.
- **Edge cases, as built:** setting off drops the in-memory state and writes nothing. A process that exits mid-hour keeps the bytes counted up to its last sample, and is forgotten. A counter lower than the last one drops the sample and keeps the higher baseline, so a glitch can undercount but never counts twice. A pid reused by a new process is a new key.
- **Edge cases, first sight and ownership:** a process first seen after watching began counts from zero; an older one counts from its first reading. Another user's process never reaches the roll-up, and a pid that moves to another user drops our stored counters. Erase and the hook share one lock, so bytes pending before an erase are not written back.
- **Lifetime SSD writes: readable without root (verified 2026-11-27, macOS 27, MacBookPro18,3, APPLE SSD AP0512R).** The drive's registry entry says `"NVMe SMART Capable" = Yes` and names `NVMeSMARTLib.plugin`. A C probe built against the SDK's `NVMeSMARTLibExternal.h`, run as uid 501, read the SMART page with return code 0 and Data Units Written 406,048,281 (about 208 TB). `iokit::ssd_lifetime_bytes_written` reads the same page from Rust and the running server returned 207,918,157,824,000 bytes. A Mac whose drive does not offer the plug-in gets the figure hidden with the reason.
- **Tests:** 17 new Rust tests and 8 Node tests.
  - `history::disk_writes`, 12: roll-up per app group, exit mid-hour, started while watching, counter backwards, reused pid, setting off, flush timing and a missing database.
  - The same module also tests that an upsert adds, the 7-day prune, erase with bytes still pending, the reply's 24-hour layout and an unreadable database.
  - `procusage`, 2: only own processes, a pid moving to another user. `iokit`, 2: SMART field parsing and saturation, a real read that is a value or a reason. `settings`, 1.
  - Node: `tests/web/storage-writes.test.mjs`.
- **Real run:** the debug and release binaries with an isolated `HOME`. With the setting off no tick ran and the table held no rows. Turned on in Chrome with the panel's button, the first rows landed after one minute (43 rows, busiest app 103 MB). The panel showed the busiest eight apps with bars and traces, "35 more apps wrote less", the 208 TB lifetime line and the footnote. Erase history emptied the table and the panel went back to its empty state; Turn off showed the off state. `POST /api/storage/writes/erase` without the token returned 403.
- **Found and fixed during the real run:** the panel first stayed on "Reading..." when opened from the Overview, because it shared the capacity history's one-minute clock, which the Overview had just reset. It now has its own clock.
- **Pending:** the 24-hour acceptance check (non-zero writes for the busiest app after 24 hours on) needs a day of running. The narrow-width layout was not checked: resizing the browser window did not change the viewport. The panel's look was not compared against design D in the owner's canvas.

