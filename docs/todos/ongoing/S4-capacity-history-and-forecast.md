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

- [ ] Tests for the forecast first.
- [ ] Write the daily sample at first scan after local midnight; skip if one exists for the day.
- [ ] Chart as inline SVG with `role="img"` and an `aria-label` that states the forecast.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Fewer than 7 days of data | "Collecting history", no forecast |
| Flat or shrinking usage | "Not growing", no days figure |
| Large one-day drop (a big clean-up) | Fit uses days after the drop |
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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
