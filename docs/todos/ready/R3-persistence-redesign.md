# R3: Persistence redesign

| Field | Value |
|---|---|
| ID | `R3` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11, D14 |
| Depends on | T1, N1, N3, A9 |
| ADRs | None |
| Branch | `feat/r3-persistence-redesign` |
| PR title | `feat(web): redesign Persistence around what changed since the baseline` |
| Size | M (200-400) |
| PR | — |

## Goal

Redesign Persistence as on board "H · Persistence": a verdict title, a "Changed since baseline" list (added, changed, removed), a table with Apple items hidden by default, and an inspector that says why an item is flagged and where it is defined.

## Context

- A2 (done) lists startup items with label, kind, scope, program, trust and date added, with chips All, New, User and System. `History` keeps `persistence(path, first_seen, last_seen)`.
- "Changed" and "Removed" need data the page lacks today: a stored fingerprint of the plist and program per item, and the time an item disappeared.

## Scope

**In scope:**

- Store, per item, a hash of the plist and the program's code-signing identity, and mark `removed_at` when an item is no longer found. New columns in the `persistence` table with a migration.
- The "Changed since baseline" list from those fields, with the baseline date.
- Table with the A2 columns, filters (All, New, User, System, Unsigned) with counts, and a toggle for Apple items (hidden by default, count shown).
- Inspector: why flagged and the rule id, when it runs (login, boot, interval, keep alive), program path, plist path, signing, first seen, Related links (app, its connections, a Guide entry), and "Mark as expected" from A9. Reveal in Finder and Copy path buttons.

**Out of scope** (name the todo that covers it):

- Disabling or removing a startup item: not planned; it needs its own safety review (A2).

## Target files

| File | Change |
|---|---|
| `src/persistence.rs` | read run-at-load, keep-alive and start interval from the plist; fingerprint |
| `src/history.rs` | new columns, migration, `removed_at` |
| `web/index.html` | `persistence()` and helpers |

## Steps

- [ ] Migration test first: an old database gains the columns without losing rows.
- [ ] Parse the plist keys with the existing `plutil` path; handle plists that fail to parse.
- [ ] Build the diff list and the inspector.
- [ ] Check in a real browser at 1360 px and 400 px.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Plist cannot be parsed | Item listed with "unreadable" and the reason; not dropped |
| Program path missing | Trust shows "program missing" |
| Item removed then restored | One row; the diff shows the restore |
| First run (no baseline) | Everything counts as baseline; "No changes yet" |
| Apple items | Hidden by default; count shown; included in search |

## Acceptance criteria

- Editing a plist's program path shows the item under "Changed since baseline" after the next scan.
- Deleting a plist shows it as removed, not as missing from the table without comment.

## Tests

Rust tests (6 minimum) for fingerprints, migration and removal; logic tests (4 minimum) for the diff. Layout checked by hand and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; `docs/agents/` notes on the history schema.

## Risks and open questions

- Fingerprints add writes on every scan; write only on change.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
