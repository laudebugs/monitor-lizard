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
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (pending, owner runs it)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** `src/baseline.rs` holds typed entries: `app:`, `host:`, `startup:` and `finding:<app>:<rule>#<ports>`. An old untyped file loads unchanged as apps and becomes typed on the next write. Writes take a mutex and a file lock, use a unique 0600 temp file, and flush the file and its folder.

**Server.** `POST /api/approve` and `/api/unapprove` take `kind` and `key`, or `app` and `addr` for a host, behind the same Origin and token guards. A bad kind, bad key or refused rule gives 400. `GET /api/baseline` reads only and never moves a file.

**Marks and labels.** A mark never raises a trust label, because labels are computed from every finding before the marked ones are split off. A finding mark covers exactly the exposed ports it was made on, so a new port brings the finding back. New hosts are marked host by host.

**Never expected.** `unsigned-network` and `debug-port` are refused at write time, as are unknown rules and `unreviewed`. Marked findings are listed in a collapsed Overview section with their scope and an Unmark button, and the heading counts them.

**Audit and limits.** The `baseline_changes` table is migration step 2, created with `IF NOT EXISTS`. An early development copy of the table lacked the `note` column, so opening the database adds it if missing. Each change is written to it before it takes effect. A failed save adds a second row saying so. The baseline holds at most 5,000 entries, the audit keeps the newest 10,000, and keys with control or invisible formatting characters are refused.

**Corrupt files.** A corrupt file is moved to `.bad`, `.bad-1` and so on, never over an older copy. The Overview shows a banner until dismissed, saying earlier marks are not in effect. An unreadable file pauses marking and is never replaced.

**Startup items.** A real change to a startup item patches the cached list at once and rescans in the background. A repeated request does nothing.

**Interface.** The Persistence and Network tables have an action column that is drawn and aligned but never exported or offered in the column chooser. Buttons sit on the Overview cards, the Persistence page and the Network page. The server sends each finding's mark key and each group's expected hosts, so the page builds no baseline keys. The History page lists every change, including failed saves.

**Tests.** 32 new Rust tests. The suite passes with 170 run and 2 ignored, plus 31 Node tests. Two real processes marking at once lose no write.

**Real run.** On a scratch home I marked a listener on port 49650, then opened 49651 in the same process. The finding returned and the label stayed Unknown throughout. The 400 cases, a corrupt file and the banner were also exercised.

**Browser.** Looked at in Chrome: Persistence, Network and History in dark, Overview in light and dark, marking and unmarking a finding, the marked section, and the banner. At 400 px the Overview banner wraps with no horizontal scroll. Forced-colors was not checked.

**Deviations.** The change also touched `snapshot.rs`, `persistence.rs`, `notify.rs`, `model.rs`, `group.rs`, `rules.rs` and `guide.rs`, which the Target files table omits. The label rule needed `snapshot.rs`, evidence needed `model.rs` and `rules.rs`, and the startup cache needed `persistence.rs`.

**Size.** The PR is about 920 changed lines against a 400 target, so the M label was exceeded. Review added the evidence scope, never-expected rules, locking, limits and tests. Splitting it would have left the unsafe version mergeable.

**Open.** `/writing-review` is for the owner's session. `new-remote` findings cannot be marked as a finding, only host by host.
