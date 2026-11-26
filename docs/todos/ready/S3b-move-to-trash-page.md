# S3b: Move to Trash on the Storage page

| Field | Value |
|---|---|
| ID | `S3b` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D5 |
| Depends on | S2a, S3a |
| ADRs | None |
| Branch | `feat/s3b-move-to-trash-page` |
| PR title | `feat(web): preview and confirm moving storage items to the Trash` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Enable the Storage page's removal buttons: a preview from the dry run, a confirmation, a second confirmation for `yours` items, and a result summary.

## Context

- S3a serves `POST /api/trash` and its dry run; the per-run token is already in the page for other writes.

## Scope

**In scope:**

- "Review" and "Move to Trash" call the dry run first and show what would move, with sizes and any refusal reason.
- "Move to Trash" is enabled only after the preview has been shown for the current selection.
- Items labelled `yours` need a second confirmation that names the folder.
- After the move: a summary of moved and refused items, a rescan, and a link to the History entry.

**Out of scope** (name the todo that covers it):

- Anything not moved to the Trash: not planned.

## Target files

| File | Change |
|---|---|
| `web/index.html` | preview dialog, confirmation, result summary |

## Steps

- [ ] Write the preview state machine (idle, previewing, confirmed, done) as a pure function with tests first.
- [ ] Wire the buttons and the token header.
- [ ] Try the flow on a folder you created for the purpose.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Selection changes after the preview | Preview is cleared and the button disabled |
| Server refuses an item | Listed with its reason; the others still move |
| Network or server error | Nothing is shown as moved; the error text is shown |
| Escape pressed in the dialog | Closes without moving |
| Double click on the button | One request |

## Acceptance criteria

- No request moves anything before a preview for the same selection has been shown.
- The summary matches the History entry.

## Tests

Logic tests for the state machine, 6 minimum. The dialog is checked by hand in a real browser and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

README safety paragraph; plan status.

## Risks and open questions

- A stale preview is the main risk; the server's change check in S3a is the backstop.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
