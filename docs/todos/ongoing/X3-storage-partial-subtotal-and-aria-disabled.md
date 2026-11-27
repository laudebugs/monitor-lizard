# X3: Hide the safe subtotal on a partial scan and keep disabled buttons focusable

| Field | Value |
|---|---|
| ID | `X3` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | S2a, S3b |
| ADRs | None |
| Branch | `fix/x3-storage-partial-subtotal-and-aria-disabled` |
| PR title | `fix(web): withhold the safe total on partial scans and use aria-disabled on Storage buttons` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Fix two Storage page problems that the review of PR #72 found and that PR left alone. First, a partial scan shows a safe-to-remove subtotal beside the clean-ups list while the Partial banner says no such total appears until a scan completes. Second, three buttons use the `disabled` attribute, which takes them out of the Tab order, so a keyboard user never learns why they cannot be pressed. The page already does it right for Move to Trash (`aria-disabled`, with the reason in `aria-describedby`).

## Context

- [PR #72](https://github.com/laudebugs/monitor-lizard/pull/72) reworded the banner and the page copy but changed no behaviour; this todo holds the behaviour changes its reviews found.
- [B7](../ongoing/B7-disk-network-detail-page.md) and [S2a](../ongoing/S2a-storage-page-treemap-table-inspector.md) record the finding in their notes.
- `stTrashButton` in `web/page-storage.js` is the pattern to copy.

## Scope

**In scope:**

- `storageCleanups` in `web/page-storage.js` (about line 318) appends `safeSubtotalText(list)` to its summary whenever safe folders exist. On a `partial` scan it must say "at least" or leave the subtotal out, so the page agrees with the banner (decide with the owner; the banner promises no total).
- The Rescan button (`stActions`, about line 274), the Review all safe button (`storageCleanups`, about line 304) and the dialog's Move to Trash button (`trashButtons`, about line 492) use `aria-disabled="true"` and a click guard that announces the reason, not `disabled`.

**Out of scope:**

- The wording of the banner and dialogs (done in PR #72). Any other page.

## Target files

| File | Change |
|---|---|
| `web/page-storage.js` | the subtotal rule and the three buttons |
| `tests/web/storage-page-view.test.mjs` | cases for both |
| `tests/web/storage-trash-view.test.mjs` | the dialog's Move button keeps focus and does not move anything while it cannot |

## Steps

- [x] Write the failing tests first: a partial scan with safe folders shows no unqualified total; each of the three buttons carries `aria-disabled` and no `disabled` while it cannot act, and a click does nothing but announce why.
- [x] Make the changes above.
- [x] Check the dialog's focus trap: `trashKeys` selects `button:not([disabled])`, so a button that is only `aria-disabled` is now inside the trap. Confirm Tab order and Escape.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Partial scan, safe folders listed | No total that reads as final |
| Complete scan, safe folders listed | The total, as today |
| Rescan inside the server's minute | Button reachable by Tab, click announces the reason, no request is sent |
| Dialog in the `moving` phase | Move button cannot be activated twice |

## Acceptance criteria

- The banner and the clean-ups summary never disagree.
- Every button that cannot act on the Storage page can still take focus and says why.
- Seen in Chrome in both themes with a scratch HOME.

## Tests

Node cases in `tests/web/storage-page-view.test.mjs` and `tests/web/storage-trash-view.test.mjs`; at least one per row of the edge-case table.

## Verification

```sh
make check-docs
make check
make test
```

## Docs to update

Move this file to `done/` with an Implementation Record; drop the follow-up lines in B7 and S2a.

## Risks and open questions

- Whether a partial scan shows "at least N" or nothing. Decided: nothing, as the banner says. Lower-bound sizes can only grow, so any safe total would be a floor that reads as an answer.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27. The todo stays in `ongoing/`: its dependencies S2a and S3b are not done, and the board check refuses a done todo with an unfinished dependency.

### Built

- A partial scan states no safe total anywhere the page computes one. The clean-ups summary says "Safe total: not shown until the scan is complete."; the Review all safe announcement and the multi-select inspector's Total say the same; the section inspector and the HTML report already withheld it, and tests now pin both. A complete scan reads as before.
- The Trash dialog's figure is the server's size for the exact folders chosen, not a safe total, so it is qualified rather than withheld: a folder the scan only partly measured makes it read "at least".
- Rescan, Review all safe and the dialog's Move button use `aria-disabled="true"` with the reason in `aria-describedby`, not `disabled`. A click, Enter or Space does nothing but announce why. Rescan also shows its reason inside the server's minute, and takes focus back after a click because `render()` rebuilds it. `trashKeys` already selected `button:not([disabled])`, so the Move button joins the Tab loop with no change there. The existing disabled styles (including forced colors) already named `[aria-disabled=true]`.
- Tests: `tests/web/storage-page-view.test.mjs`, `storage-trash-view.test.mjs`, `storage-cleanups.test.mjs`.

### Checked in Chrome

A scratch server (scratch `HOME`, no window resize) with `window.fetch` replaced by scripted replies: partial scan, complete scan, failed load, a scan running, and a Rescan inside the minute. With real key presses: Shift+Tab reached Rescan, Enter and Space sent no request and kept focus; in the Trash dialog Tab reached "Nothing can be moved", Enter did nothing, Escape closed it. Dark and light were looked at; forced colors was not run in a browser.
