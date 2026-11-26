# A9: Mark as expected (feedback baseline)

| Field | Value |
|---|---|
| ID | `A9` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), risk 6; [system-monitor](../../plans/system-monitor.md) section 5 |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a9-mark-as-expected` |
| PR title | `feat(baseline): mark hosts, startup items and findings as expected` |
| Size | M (200-400) |
| PR | — |

## Goal

Let the user say "this is expected" for a remote host, a startup item or a single finding, so it stops showing as new or needing a decision. Today only whole apps can be approved (`src/baseline.rs`, a JSON list of group keys).

## Context

- `baseline::approve(key)` writes `~/.monitor-lizard/baseline.json`; `POST /api/approve?key=` exists.
- `History` holds `seen` (first-seen hosts per app) and `persistence` (startup items by path).
- Design boards F, G and H have "Mark expected" and "Mark host as expected" buttons.

## Scope

**In scope:**

- Baseline entries become typed: `app:<key>`, `host:<app-key>|<addr>`, `startup:<plist path>`, `finding:<id>`. Existing entries load as `app:`.
- `POST /api/approve?kind=host&key=...` with the same Origin and per-run token guards as other writes.
- Marked hosts and startup items drop out of the New filters and the Overview list; they stay in their tables with an "expected" chip.
- `monitor-lizard approve` accepts the typed form and keeps the old form working.
- Each marking is recorded in a new `baseline_changes` table (time, kind, key, mark or unmark), created with `CREATE TABLE IF NOT EXISTS`, because the `actions` table stores only pids. An "Unmark" action undoes it.

**Out of scope** (name the todo that covers it):

- Muting for a time (the designs show "Mute 1 day"): a later todo.

## Target files

| File | Change |
|---|---|
| `src/baseline.rs` | typed keys, migration, `unapprove` |
| `src/history.rs` | `baseline_changes` table and writer |
| `src/server.rs` | `kind` parameter, `POST /api/unapprove` |
| `src/main.rs` | CLI form |
| `web/index.html` | buttons and chips (restyled by T1a, T1b, R1, R2 and R3b) |

## Steps

- [x] Tests for migration of an old `baseline.json` first.
- [x] Add typed keys and the server routes.
- [x] Wire rules so an expected host or startup item no longer produces a finding.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Old baseline file | Loads unchanged; entries read as apps |
| Unknown `kind` | 400 |
| Marking an item that is not on the machine | Allowed, kept until unmarked |
| Missing token or wrong Origin | 403 |
| Same key twice | One entry |
| Corrupt baseline file | Backed up as `baseline.json.bad`, empty baseline used, user told |

## Acceptance criteria

- Marking a new host as expected removes it from "New" and from Overview without hiding its row.
- Unmarking returns it.

## Tests

Rust tests, 10 minimum.

## Verification

```sh
make check
make test
```

## Docs to update

`SECURITY.md` (what the baseline means), README, plan status.

## Risks and open questions

- Marking a malicious item expected hides it from the Overview. The row keeps an "expected" chip and the History log shows when it was marked.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md` (pending, owner runs it)
- [ ] `/writing-review` run on any prose (pending, owner runs it)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** `src/baseline.rs` holds typed entries: `app:`, `host:`, `startup:` and `finding:`. An old untyped file loads unchanged as apps and becomes typed on the next write. A corrupt file is moved to `baseline.json.bad`, an empty baseline is used, and the dashboard shows a notice.

**Server.** `POST /api/approve` and `/api/unapprove` take `kind` and `key`, behind the same Origin and token guards. An unknown kind or bad key gives 400. `GET /api/baseline` lists entries. Each change goes into the new `baseline_changes` table first, and nothing changes if that write fails.

**Effect.** Expected hosts leave the new-hosts finding. Expected startup items are never new. Expected findings move to `expected_findings` in the snapshot, out of counts, notifications and labels. The CLI `approve` takes typed keys.

**Interface.** Buttons sit on the Overview cards, the Persistence page and the Network page. Marked rows keep an "expected" chip and an Unmark button. The History page lists every change.

**Tests.** 18 new Rust tests. The suite passes with 156 run and 2 ignored, plus 31 Node tests. A real run on a scratch home checked 403, 400, repeated marks and the audit rows.

**Browser.** Looked at in Chrome: Persistence, Network and History in dark, Overview in light, marking and unmarking a startup item, a host and a finding. At 400 px only the sidebar fits, so the new columns were not assessed there.

**Deviations.** Unsigned-network findings cannot be marked, so approving cannot hide an unsigned program; the todo did not say this. `baseline_changes` is created by `CREATE TABLE IF NOT EXISTS` as asked. Once the schema-version PR merges, it needs to become a migration step. Muting is out of scope.

**Open.** `/code-review` and `/writing-review` are for the owner's session. The History page shows marks but offers no Unmark there. Forced-colors and the 400 px view of the new buttons are unchecked.
