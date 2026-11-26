# S5: Per-app disk writes (opt-in)

| Field | Value |
|---|---|
| ID | `S5` |
| Status | `ready` |
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

- [ ] Tests for roll-up and counter resets first.
- [ ] Change `settings::update` from four fixed parameters to a struct, so a new setting does not need another parameter.
- [ ] Key per-process counters by pid and start time so a reused pid is not mixed in.
- [ ] Add the erase route behind the same Origin and token checks as other writes.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
