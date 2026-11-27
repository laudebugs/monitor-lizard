# U6b: Migrate Guide, Overview, Apps and Privilege

| Field | Value |
|---|---|
| ID | `U6b` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U6a |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6b-medium-pages` |
| PR title | `refactor(web): run Guide, Overview, Apps and Privilege on the Preact renderer` |
| Size | L (over 400: split by page if needed) |
| PR | — |

## Goal

The same process as U6a for the four pages with more state: the Guide (it skips redraws with `gsig` and announces changes), Overview, Apps (detail drawer, resizable handle, `data-req` focus) and Privilege (requester chain, detail).

## Context

- `web/page-guide.js` (`guideAct`, `gsig`), `web/page-apps.js`, `web/page-privilege.js`, `web/page-overview.js`; `web/table.js` drawer handle (`hadFocus`, `.drawer`).
- On the Preact renderer the `gsig` skip and `guideAct` are unnecessary; the announce logic stays.

## Scope

**In scope:**

- Per page: DOM diff, keys, escaping tests, delete `guideAct`, `data-req` and the nav `had` restore on the Preact renderer, make the drawer survive commits (it writes `--dw` and a `sized` class on a live element: move the width into `S` and a `style` prop if it fights Preact).

**Out of scope** (name the todo that covers it):

- Performance (U6c), Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-guide.js`, `web/page-apps.js`, `web/page-privilege.js`, `web/page-overview.js`, `web/table.js`, `web/shell.js` | keys, neutral helpers, drawer, nav |

## Steps

- [ ] Guide: type in the search box, pick a command, keep focus through a poll; announcements fire only on a real change.
- [ ] Apps: select an app, open the drawer, drag the handle, press Stop (dialog from U5) while a poll arrives.
- [ ] Privilege: open a request chain; copy the chain text and JSON.
- [ ] Overview: badges and links open the right filtered pages.
- [ ] Sidebar: Tab to a nav button and let a poll arrive; focus stays.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Drawer resized, then a commit | width kept |
| Guide search with no match | the empty state with its button, focus kept |
| Escape in the drawer | as today |
| An app disappears while selected | `selPatch` handling as today |
| Typing in `#q` | the box keeps focus (it is outside the container) |
| A hostile app or command name | escaped by `visible()` |
| Column menu open when a poll arrives | stays open (`quiet()` still guards; U7 replaces it) |

## Acceptance criteria

- All four pages match the old renderer in DOM and behaviour; `guideAct`, `data-req` and the nav focus restore are unused on the Preact renderer.
- **Maintainability (the reason for this plan):** each page or component moved deletes the workarounds it needed (a `data-req` or `stFocus`-style focus restore, a `quiet()` guard, a dataset-cached state) and the PR description records before and after counts of those workarounds and of lines in the files touched. Logic that a page owns (labels, rows, text) is tested in Node against the vnode stand-in, with no browser needed; only behaviour that needs a real browser (focus, `inert`, scroll) is checked by hand.

## Tests

Markup, key and escaping cases per page; browser checks recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

`docs/agents/layout.md` "Adding a dashboard page" row once the pattern is stable.

## Risks and open questions

- The drawer handle mutates live elements; it is the likeliest place for a foreign-mutation fight.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** The default is the old renderer; only the Preact renderer changes.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
