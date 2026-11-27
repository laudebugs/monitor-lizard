# S3b: Move to Trash on the Storage page

| Field | Value |
|---|---|
| ID | `S3b` |
| Status | `ongoing` |
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

The server contract is plan D15 (S3a follow-up); S3b must use it as follows:

- **Preview:** `POST /api/trash?ids=a,b` (no `dry`, or `dry=1`). The reply has `confirm_token` when something would move, and `items[]` with `status`, `code` and `reason` for each id. Send `confirm_yours=1` on the preview too when the user has already given the second confirmation: the token is bound to it. A preview with nothing movable has no token.
- **Real move:** `POST /api/trash?ids=a,b&dry=0&confirm_token=...` with exactly the same ids (and the same `confirm_yours`). The token is single use and lasts 10 minutes. Errors: 400 `confirmation_required`, 409 `confirmation_invalid`, `confirmation_expired`, `confirmation_mismatch`, 409 `busy` (another removal is running), 429 `too_many_previews`, 500 `audit_unavailable`.
- **Reply:** `ok` (true only if everything moved), `history_id` (the audit entry to link to), `counts`, per-item `status` of `moved`, `refused`, `failed` or `not_attempted`, `failed` (the item whose move failed) and `note` (Put Back). All items refused is a 200 with `ok:false`; a 500 with a body still lists what moved.
- **Page rules:** disable the button on click (one request); keep the token only with the selection it was issued for and clear it when the selection or the yours confirmation changes; name the folder in the yours confirmation; show that Finder's Put Back may not be offered and that History keeps the original path; show every non-moved item with its `reason`.


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

- [x] Write the preview state machine (idle, previewing, confirmed, done) as a pure function with tests first.
- [x] Wire the buttons and the token header.
- [ ] Try the flow on a folder you created for the purpose. Outstanding: the owner does this; see the Implementation Record.

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
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27. Not done: the todo stays in `ongoing/` because its acceptance step, a real move of a throwaway folder, was deliberately not run by the agent.

### Built

- `storage-trash` block in `web/page-storage-logic.js`: the pure state machine (`trashStep`) and helpers, with 24 Node tests in `tests/web/storage-trash.test.mjs`, written before the dialog.
- The dialog in `web/page-storage.js` (`storage-trash-view` block), mounted from `web/shell.js` through `trashLayer`: focus moves to the heading, Tab wraps inside it, the page behind is `inert`, Escape and Cancel close it without a request (not while a move runs), and one Escape closes one layer. Every folder name, reason, code, note and audit error passes through `visible()`, so a bidi override, newline or zero-width character shows as an escape. A dialog that cannot be drawn closes with a message. Styles are in `web/app.css` with a forced-colors block.
- `api()` in `web/data.js` now attaches `status` and `text` to the error it throws. Nothing else changed in how it works.
- The Move to Trash button stays `aria-disabled` with helper text when nothing previewable is selected. It now also appears for a single `yours` folder (it used to say no removal is offered); that path asks for the second confirmation naming the folder.
- Order for `yours`: preview, "Continue", a confirmation that names the folder, a new preview with `confirm_yours=1`, then the move with the same flag. The first token is dropped when the confirmation is given.
- A Rescan is requested after a result. The server reuses a scan under a minute old, so the list can still show moved folders until a later Rescan; the dialog says so.

### Deviations and limits

- The History page now lists `trash_moves` (section "Moves to the Trash", one row per item: when, request, folder, state, size, why) with the usual sort, search, export and column rules. Interrupted, pending, moving and unreadable rows are shown as what they are, and an unknown state is shown as unknown, never as moved. The dialog's "Open History" link goes there and names the request number.
- `quiet()` (background polls) no longer redraws while the dialog is open, as it already did for the Stop dialog.
- Agent testing never called `POST /api/trash` on a real server. The page ran against a scratch server (a scratch `HOME`) with `window.fetch` replaced in the console by scripted replies for: preview ok with a token, a yours preview with refusals, the yours re-preview carrying `confirm_yours=1`, commit ok (double click sent one commit), partial failure, 409 busy, expired token, network error, a 500 with a body, all items refused, and Tab and Escape handling. Dark and light were looked at; forced colors was not run in a browser.

### Manual check for the owner (not run)

1. Make a throwaway folder in your home, for example `~/Library/Caches/monitor-lizard-s3b-test` with a few small files, and Rescan on the Storage page.
2. Select only that folder. Check the preview, then try Escape (nothing moves), reopen, and confirm. Check the result, that the folder is in the Trash, and that `GET /api/history` shows the same entry as the dialog.
3. Repeat with a folder labelled yours (the confirmation must name it) only if you are happy to move one.

### Review follow-up (same PR)

- Escaping: all folder names and server text in the dialog and in History go through `visible()`; tests render the dialog with RLO, newline, zero-width, `<` and `&` and check the output (`tests/web/storage-trash-view.test.mjs`).
- The preview key now holds each id with its label, so a rescan that makes a folder YOURS clears the preview instead of committing without `confirm_yours`.
- Closing paths: a changed selection or label, leaving the page, and a dialog that cannot be drawn each close the dialog with an announcement. The first two return focus to the trigger (or the main region). Not every state change is announced; the preview, the move, the result and each close are, a Go back from the yours confirmation is not.
- The server's `ok`, `counts`, `failed` and `moved` are checked against the item list. A disagreement shows a visible warning pointing to History; the item list is still what is counted.
- `/api/history`'s request id and the dialog's "Recorded as request N" are the same number; `tests/web/storage-trash.test.mjs` builds History rows from the same reply and compares the counts and moved bytes with the summary.
- The scan reuse claim in the dialog was checked against `src/storage/cache.rs`: `MIN_INTERVAL` is 60 seconds and a request inside it gets the cached scan.
- Size: the web change is about 360 added lines (S budget is under 200). It grew because the History table, the escaping and the dialog guards were added after review.
- Forced colors: the CSS only sets borders and uses system colours, and every status is also text; it was not run in a browser.
