# U6b: Migrate Guide, Overview, Apps and Privilege

| Field | Value |
|---|---|
| ID | `U6b` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U6a |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6b-medium-pages` |
| PR title | `refactor(web): run Guide, Overview, Apps and Privilege on the Preact path` |
| Size | L (over 400: split by page if needed) |
| PR | — |

## Goal

Same process as U6a for the four pages with more state: the Guide (it skips redraws with `gsig` and announces changes), Overview, Apps (detail drawer, resizable handle, `data-req` focus) and Privilege (requester chain, detail).

## Context

- `web/page-guide.js` (`guideAct`, `gsig`), `web/page-apps.js`, `web/page-privilege.js`, `web/page-overview.js`; `web/table.js` drawer handle (`hadFocus`, `.drawer`).
- The Guide's `gsig` skip exists to avoid losing focus; on Preact it can go, but its announce logic stays.

## Scope

**In scope:**

- Per page: DOM diff, keys, remove `guideAct` and `data-req` focus code on the Preact path, drawer resize still works (it writes `--dw` on the live element: it must survive a commit), tests for markup helpers.

**Out of scope** (name the todo that covers it):

- Performance (U6c), Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-guide.js`, `web/page-apps.js`, `web/page-privilege.js`, `web/page-overview.js`, `web/table.js` | keys, neutral helpers, drawer survives commits |

## Steps

- [ ] Guide: type in the search box, pick a command, keep focus in the box through a poll; announcements still fire only on a real change.
- [ ] Apps: select an app, open the drawer, drag the handle, press Stop (dialog from U5), poll arrives.
- [ ] Privilege: open a request chain, copy the chain text and JSON.
- [ ] Overview: badges and links open the right filtered pages.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Drawer resized, then a commit | width kept (`--dw` and `sized` class survive) |
| Guide search with no match | the empty state with its button, focus kept |
| `Escape` in the drawer | as today |
| App disappears while selected | selection handling as today (`selPatch`) |
| Search typed in `#q` | the box keeps focus (it is outside the container) |

## Acceptance criteria

- All four pages match the old path in DOM and behaviour; `guideAct` and `data-req` are unused on the Preact path.

## Tests

Markup cases per page; browser checks recorded.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md` "Adding a dashboard page" row once the pattern is stable.

## Risks and open questions

- The drawer handle code mutates live elements; if it fights Preact, move its width into `S` and a style attribute.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR.
- **Why the page works at every step:** Old path is the default; only the opt-in path changes.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
