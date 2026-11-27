# T4: A mobile-width layout direction

| Field | Value |
|---|---|
| ID | `T4` |
| Status | `done` |
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
- The sidebar (`.side`/`.main` flex row in `web/index.html`) has no collapsed/drawer form at all yet. At a phone width it sits beside the content, each squeezed, rather than stacking or hiding behind a toggle; this was first noticed firsthand while testing B6/B8 in this session, not filed as its own bug. **Correction, recorded while building this todo:** that guess was wrong. Measured in a real browser, the shell's own `flex-wrap` already stacks the sidebar above the content with no page-level overflow below about 800px -- nothing is squeezed side by side. The real problem is that the stacked sidebar (nav list, logo, Notify/Theme controls) is about 708px tall, so a phone-height viewport showed almost nothing but navigation on first load; reaching any page content took a long scroll. See the Implementation Record.
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

- [x] Pick and record the breakpoint and the pattern for the sidebar, top bar, tiles, tabs and dialogs (a short written design pass, screenshotted at the target width, before or alongside the CSS).
- [x] Implement the sidebar collapse/drawer and top-bar changes.
- [x] Re-check every table (not just the two already-fixed ones) at the new breakpoint and at a true phone width (390 px), fixing what the audit finds.
- [x] Re-check the Stop/Quit dialog and the Performance tiles/tabs/charts at the same widths.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `web/index.html`: a `#navtoggle` hamburger button added to `.top` (before `#crumb`), and `id="side"` on the sidebar `<nav>` so the button's `aria-controls` is real. `web/core.js`: `navOpen: false` added to the shared state object `S`. `web/app.css`: `.navtoggle`/`.navbackdrop` (the latter reuses `.overlay`'s position/background/padding, only overriding `z-index`); `@media(max-width:860px)` makes `.navtoggle` visible and `.side` a fixed, off-canvas drawer (`transform:translateX(-100%)` by default, `.shell.nav-open .side{transform:translateX(0)}`), with `prefers-reduced-motion:reduce` dropping the transition. `web/shell.js`: `openNav`/`closeNav` toggle `S.navOpen` and move focus (opening focuses the first nav item; closing returns focus to the toggle); a nav item's own click closes the drawer and focuses `#main` instead (matching the existing skip-link target, since picking a page should move focus into that page, not back to the button); a `resize` listener closes a stray-open drawer if the window widens past 860px while it is open; `render()` syncs `.shell`'s `nav-open` class and the toggle's `aria-expanded`. `web/page-storage.js`: `closeTopLayer` (the existing single Escape-dispatcher, already spanning the Trash dialog, the Signal dialog and the Stop modal) gained the open drawer as its lowest-priority case.
- **Design pass (done before the CSS, per Scope):** measured the real narrow-width behavior first rather than trust this file's own Context guess, which turned out to be wrong -- see the Context correction above. The actual problem (a ~708px-tall stacked sidebar pushing all content below the fold on a phone-height screen) led to the chosen pattern: collapse the sidebar behind a toggle into a fixed off-canvas drawer with a backdrop, rather than relying on the existing `flex-wrap` stack. Breakpoint: 860px, reusing the exact width the shell's own pre-existing `@media(min-width:861px){.side{position:sticky}}` rule already treated as "wide enough for the sidebar beside the content," so there is one breakpoint, not two close together.
- **Tests:** no new pure-logic module (this is CSS/DOM structure, consistent with the Tests section's own expectation); the existing 352 web tests and 485 Rust tests still pass, confirming no regression. Manual real-browser checks below stand in for automated coverage, same as other CSS-only todos (e.g. T1d).
- **Real run:** built the release bundle, served it with a scratch `HOME`, and drove it through the Claude in Chrome extension using same-origin iframes at each target width (the extension's own window-resize tool does not change the viewport in this environment -- the same limitation recorded in B8's Implementation Record). Automated a sweep of all 10 pages (Overview, Apps, Privilege, Listeners, Persistence, Network, History, Performance, Storage, Guide) at 390px, 860px and 1440px in both themes (60 checks): zero show `document.documentElement.scrollWidth` greater than the viewport width, so the pre-existing tables are not re-broken by the sidebar change. Confirmed by hand: at 390px the sidebar starts fully off-canvas and `.main` content begins at the very top of the viewport (previously it started 708px down); the toggle opens the drawer (focus moves to the first nav item), Escape, a backdrop click and a nav item click each close it and return focus correctly (to the toggle, or to `#main` after navigating); widening the window past 860px while the drawer is open auto-closes it; above 861px the toggle is hidden and the sidebar is sticky exactly as before, in both themes, at 859px vs. 861px. The Performance page's tiles stack to one column and its tabs wrap sensibly at 390px with no overflow. The Stop/Quit confirm dialog was checked at 390px and needed no change: it was already fully visible with reachable buttons, via its own existing `.overlay{padding:20px}` and the dialog's flex-wrapping button row.
- **Review (2026-11-27):** `/code-review high 5` ran a standards and a spec review in parallel, against `4322c26`. Standards findings fixed: `docs/agents/layout.md` was not updated (added, see Built); `.navbackdrop` duplicated `.overlay`'s rule almost verbatim (now shares the class, overriding only `z-index`); three slightly different inline shapes for opening/closing the drawer were consolidated into named `openNav`/`closeNav` functions (the nav-item click stays its own shape, since it closes-and-navigates-and-focuses-differently, not just closes); the 860px breakpoint, a bare literal in both CSS and JS with no way to share a single source of truth across languages without a build step this app deliberately has none of, now carries a comment in each file pointing at the other. Judgement call not adopted, with reason: `closeTopLayer` reaching into shell-level `S.navOpen`/`closeNav` was flagged as Feature Envy, but that function is already the one established Escape-dispatcher spanning several unrelated concerns (the Trash dialog, the Signal dialog, the Stop modal); adding the drawer as a fourth case is consistent with its existing, already-accepted role, not a new problem this todo introduced -- relocating that dispatcher is a larger, separate refactor. Spec findings fixed: `docs/agents/layout.md` (also a Standards finding, above); no Implementation Record existed yet at review time (this section); the Steps/Acceptance-criteria real-browser checks had not yet covered every table, both themes, or 1440px/860px/390px as asked -- the full sweep described in Real run above was run afterward to close this gap.
- **Deviation:** none beyond the Context correction already described.
- **Open:** Apps, Listeners, Privilege and History still have no redesign at any width (D9's pre-existing "design gap," unchanged by this todo). T2 is merged; T3 (PR #76) is open but not yet merged as this branch was built, so its `docs/agents/layout.md` `table.js` row (the `colresize` addition) is not in this branch's base -- expect a trivial, non-conflicting merge of that one doc paragraph whichever of T3 or this lands second.
- **CI:** pending on the pull request.
