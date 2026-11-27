# T4: A mobile-width layout direction

| Field | Value |
|---|---|
| ID | `T4` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | T1b |
| ADRs | None |
| Branch | `feat/t4-mobile-responsive-layout` |
| PR title | `feat(web): a usable layout at phone widths` |
| Size | M (200-400), may split once a design exists |
| PR | — |

## Goal

The dashboard is usable down to a phone-width viewport (about 390-430 px), not just "does not break": the sidebar, tables, Performance charts and dialogs all have a defined narrow layout, not just whatever the desktop grid happens to do when squeezed.

## Context

- Today's narrow-width work has been reactive, one table at a time, filed as individual bugs once found. `docs/bugs/network-table-columns-collapse-at-narrow-width/` (columns shrank to one character) and `docs/bugs/storage-lists-widen-the-page-at-narrow-width/` (the page itself scrolled sideways) are both fixed, but only for the tables they covered.
- The sidebar (`.side`/`.main` flex row in `web/index.html`) has no collapsed/drawer form at all yet. At a phone width it sits beside the content, each squeezed, rather than stacking or hiding behind a toggle; this was first noticed firsthand while testing B6/B8 in this session, not filed as its own bug.
- **No approved design for a mobile layout exists in the owner's design canvas yet** (D9 names only the desktop "Sys Monitor Redesign Directions" boards). This is explicitly a design-then-build todo, not a build-from-an-existing-design one like R1/R2/R3b.

## Scope

**In scope:**

- A real breakpoint (not just "whatever fits"): below it, the sidebar collapses behind a menu button instead of sitting beside the content; the top bar, breadcrumb and search adapt to the narrower width.
- Every existing table's narrow-width behavior (scroll-inside-card, column floors) audited against the new breakpoint, not just the two already-fixed pages.
- The Performance page's tiles, tabs and charts at phone width: tiles stack to one column, tabs wrap or scroll, charts stay readable (ties into B10 if it has landed, otherwise the current non-interactive charts).
- The Stop/Quit confirm dialog and any other modal at phone width: full-width or near-full-width rather than a fixed desktop size that could overflow.
- A short design note (in this file's own Implementation Record, or a linked doc) recording the breakpoint and the chosen pattern for each area above. This makes it reviewable before or alongside the code, the same way other redesign todos point at an external board.

**Out of scope** (name the todo that covers it):

- Apps, Listeners, Privilege and History redesigns: still blocked on D9's "design gap," same as R4.
- Touch-specific interactions (swipe, long-press) beyond making existing clickable things big enough to tap: not asked for.
- The Tauri shell window's own minimum size: A17 already fixes the shell's own window size; this is about the browser-served dashboard at any width, including a phone browser hitting it directly.

## Target files

| File | Change |
|---|---|
| `web/index.html` (or its split files once T2 merges) | Sidebar collapse/drawer, breakpoint CSS, tile/tab/dialog adjustments |
| `docs/agents/layout.md` | Note the new breakpoint and where its CSS lives |

## Steps

- [ ] Pick and record the breakpoint and the pattern for the sidebar, top bar, tiles, tabs and dialogs (a short written design pass, screenshotted at the target width, before or alongside the CSS).
- [ ] Implement the sidebar collapse/drawer and top-bar changes.
- [ ] Re-check every table (not just the two already-fixed ones) at the new breakpoint and at a true phone width (390 px), fixing what the audit finds.
- [ ] Re-check the Stop/Quit dialog and the Performance tiles/tabs/charts at the same widths.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Sidebar collapsed, a nav item clicked | Collapses the drawer back and navigates, same as a desktop click |
| A table already fixed for narrow widths (Network, Storage lists) | Still correct at the new breakpoint; not re-broken by the sidebar change |
| The Stop/Quit dialog at 390 px | Fully visible, no horizontal overflow, buttons reachable without scrolling the dialog itself |
| Keyboard-only navigation with the sidebar collapsed | The menu toggle is reachable and operable by keyboard, same as every other control |

## Acceptance criteria

- Every page is usable (nothing clipped, nothing requiring horizontal page scroll) at 390 px and at the chosen breakpoint, in a real browser, both themes.
- The design pass from Steps is recorded (even informally) before the CSS that implements it, so a reviewer can compare the two.
- UI layout and interaction checked in a real browser against the chosen design at 1440 px, at the breakpoint, and at 390 px (record what was tested, or mark pending).

## Tests

No new pure-logic module is expected (this is primarily CSS and layout structure); any pure logic split out (for instance a breakpoint-matching helper) gets a jsdom smoke test the way other such helpers do.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- This is explicitly design-then-build, unlike the R-series todos: there is a real risk of building something the owner then wants changed. The written design-pass step exists to catch that before the CSS is deep, not to replace the owner's own review.
- T2 (splitting `web/index.html`) and T3 (resizable columns) both touch the same table/shell code this todo touches; whichever lands last should expect to re-apply its own changes on top of the others, per `docs/agents/layout.md`.
- The two already-filed narrow-width bugs were found and fixed per-page, after the fact; this todo's audit step is meant to catch the rest before they become their own bug folders, but may not be exhaustive.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
