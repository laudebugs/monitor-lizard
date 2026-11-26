# A9: Mark as expected (feedback baseline)

| Field | Value |
|---|---|
| ID | `A9` |
| Status | `ready` |
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

- [ ] Tests for migration of an old `baseline.json` first.
- [ ] Add typed keys and the server routes.
- [ ] Wire rules so an expected host or startup item no longer produces a finding.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
