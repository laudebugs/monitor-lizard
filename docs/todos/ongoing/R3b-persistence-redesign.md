# R3b: Persistence page redesign

| Field | Value |
|---|---|
| ID | `R3b` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11, D14 |
| Depends on | R3a, T1b, N1, N3, A9 |
| ADRs | None |
| Branch | `feat/r3b-persistence-redesign` |
| PR title | `feat(web): redesign Persistence around what changed since the baseline` |
| Size | M (200-400) |
| PR | — |

## Goal

Redesign Persistence as on design board "H · Persistence". It has a verdict title, a "Changed since baseline" list, a table with Apple items hidden by default, and an inspector that says why an item is flagged.

## Context

- R3a supplies the schedule, fingerprint, removal time and `changes` list.
- A2 (done) built the current table with chips All, New, User and System.

## Scope

**In scope:**

- Title from the counts ("1 new startup item since your baseline", or "No changes since your baseline").
- "Changed since baseline" list with ADDED, CHANGED and REMOVED chips and relative times.
- Table with the A2 columns and filters All, New, User, System and Unsigned, each with a count, plus a toggle for Apple items (hidden by default, count shown). N3 adds column choice and export.
- Inspector: why it is flagged and the rule id, when it runs, program path, plist path, signing, first seen, Related links (app, its connections, a Guide entry) and "Mark as expected" from A9. Copy path and Reveal in Finder buttons.

**Out of scope** (name the todo that covers it):

- Disabling or removing a startup item: not planned; it needs its own safety review.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `persistence()` and helpers |

## Steps

- [ ] Write the title and filter-count functions with tests first.
- [ ] Build the list, table and inspector on the shared shell.
- [ ] Check in a real browser at 1360 px and 400 px.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Item marked expected | Chip "expected"; leaves New and the change list |
| Apple items hidden | Count shown; included in search results |
| Unreadable plist | Row shows the reason in the inspector |
| No changes | Change list shows "No changes since your baseline" |
| Reveal in Finder | Opens the item's folder only; never runs the program |

## Acceptance criteria

- The title count equals the number of rows under the New filter.
- Every inspector field comes from `/api/persistence`, with nothing computed from guesses.

## Tests

Logic tests, 6 minimum. Layout checked by hand in a real browser and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; the Persistence section of the README.

## Risks and open questions

- Reveal in Finder needs a write-free server route or a `file://` link; check which works without a new permission prompt.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
