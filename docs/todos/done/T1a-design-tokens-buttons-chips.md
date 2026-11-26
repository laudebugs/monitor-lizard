# T1a: Design tokens, buttons and chips

| Field | Value |
|---|---|
| ID | `T1a` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11 |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/t1a-design-tokens-buttons-chips` |
| PR title | `feat(web): shared tokens, buttons, chips and filters` |
| Size | M (200-400) |
| PR | — |

## Goal

Move the dashboard's shared styles to the Console look from the design canvas: violet tokens, chamfered mono-caps buttons, square chips and filter controls, and a matching light theme. Sidebar groups, the shared page shell and table and inspector styles are T1b.

## Context

- Today's CSS in `web/index.html` already sets `*{border-radius:0}` and defines `--bg`, `--panel`, `--panel2`, `--line`, `--fg`, `--mut`, `--acc`, `--ok`, `--warn`, `--bad`, with a light theme under `:root[data-theme=light]`.
- Design boards D to H show the target: nav groups (Security, Machine, Help), count badges, chamfered buttons, mono caps chips, 1 px lines.

## Scope

**In scope:**

- Keep the existing token names and change values; add `--cut` (chamfer size) and chip colors.
- Buttons `.btn`, `.btn-acc`, `.btn-danger` with clip-path chamfer; focus ring preserved.
- Chips for severity, trust, scope and status; text always present, never color alone.
- A light theme with the same shapes; contrast at least 4.5:1 for text and 3:1 for chip text on its fill, checked.
- No remote assets; fonts stay the bundled IBM Plex.

**Out of scope** (name the todo that covers it):

- Sidebar groups, count badges, the page shell and table and inspector styles: T1b.
- New layouts: R1, R2, R3b. Apps, Listeners, Privilege and History keep their layouts.

## Target files

| File | Change |
|---|---|
| `web/index.html` | CSS tokens and component classes |

## Steps

- [ ] List every use of `.btn`, `.chip`, `.tag`, `.tile` and `.badge` in the file and map each to the new classes.
- [ ] Change tokens and shared classes; check each page in dark and light.
- [ ] Verify focus is visible on every control and that `clip-path` does not cut the focus ring.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Button text longer than its box | Wraps or grows; the chamfer never clips text |
| Windows High Contrast or forced colors | Buttons keep a visible edge |
| 400 px width | Sidebar stacks above the content as now |
| `prefers-reduced-motion` | No transitions |
| Light theme | Every chip and badge readable |

## Acceptance criteria

- [x] No page shows the old rounded or flat buttons. Checked by eye on all nine pages in both themes (see Browser sweep).
- [x] A contrast check of tokens passes; the result is recorded in the Implementation Record.
- [x] Every page is checked in a real browser in both themes at 795 px (the extension's window, which cannot be resized) and at 400 px (a 400 px same-origin frame), with the horizontal-scroll width measured.
- [x] Verified for these widths and pages, in a real browser: dark at 1352 px on eight pages (Overview, Apps with the inspector open, Performance overview tab, Network, Listeners, Privilege, Persistence, History); light at 1352 px on four pages (Overview, Apps with the inspector open, Persistence, Guide); both themes at 795 px on nine pages; both themes at 400 px on all pages, with the scroll width measured. The 1352 px looks were done by the owner's session, not by this agent.
- Not covered, and moved to T1c: light at 1352 px on Network, Listeners, Privilege, History and Performance; the CPU and GPU tab at 1352 px; Guide in dark at 1352 px; keyboard focus and hover at 1352 px. No page is claimed verified at 1360 px.
- Scoped out: forced-colors mode and `prefers-reduced-motion`, because the browser extension cannot switch either on and DevTools emulation was not used. Read in the CSS only. Moved to T1c.

## Tests

Existing logic tests stay green. Visual checks are manual and recorded.

## Verification

```sh
make check
make test
cargo run --release -- serve   # visit every page in both themes
```

## Docs to update

`docs/agents/layout.md` if it names the CSS; plan status.

## Risks and open questions

- Large single-file diff; T1b is split off so review stays possible.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages (in the earlier rounds, not on the sweep commits)
- [ ] `/writing-review` run on any prose (not run; the prose in this todo and in T1c is unreviewed)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26. Built and merged. The browser sweep below closes the todo; forced-colors and reduced-motion moved to T1c.

**Built** (all in `web/index.html`):

- Dark and light token sets. Existing names are kept; `--cut` and eight `--chip-*` tokens are new.
- `.btn` with `.btn-acc` and `.btn-danger`, plus `.btn-danger.quiet`, the outline form for row actions.
- Square mono-caps `.tag`, `.badge`, `.chip` and `.sel-btn`.
- A forced-colors block, and the reduced-motion rule now lists the controls.
- JavaScript changed only in class strings: `pri` became `btn-acc`, `danger` became `btn-danger quiet`, `solid` became `btn-danger`.

**How the chamfer keeps its edge.** A `clip-path` also clips borders and outlines. The edge is therefore the button's own background, and `::before` paints the fill inset by `--bw`. Focus raises `--bw` to 3px and recolors the edge, so no outline is cut.

**Contrast** (WCAG 2.x, script run on the token values; text needs 4.5, all pass):

| Pair | Dark | Light |
|---|---|---|
| fg on bg / panel / panel2 / sel | 15.54 / 15.10 / 14.23 / 13.44 | 16.14 / 17.47 / 14.80 / 14.39 |
| mut on bg / panel / panel2 / sel | 7.46 / 7.26 / 6.84 / 6.46 | 6.41 / 6.94 / 5.88 / 5.72 |
| acc on panel | 6.96 | 6.40 |
| accfg on acc | 7.16 | 6.40 |
| badfg on bad | 8.29 | 5.80 |
| ok / warn / bad on panel | 11.88 / 11.56 / 8.37 | 5.34 / 5.93 / 5.80 |
| chip ok / warn / bad / neutral | 8.25 / 8.15 / 7.12 / 9.33 | 6.87 / 6.94 / 6.94 / 8.98 |

`--line` on `--panel` is 1.40 (dark) and 1.50 (light), below the 3:1 asked of a control boundary. It matches the design canvas. The label, hover edge and focus edge carry the affordance. Raising it is the owner's design call.

**Looked at in Chrome** (this branch's build served locally):

- The reviewing session saw dark Overview and Apps, and light Overview, at 1352 px.
- It saw keyboard focus on a sidebar button, with a visible outline.
- It saw a 400 px frame in both themes with no horizontal scroll; content stacks below the sidebar.
- A review found Approve and Details rendering unstyled; that was a dangling class bug, now fixed.
- After the fix, a second session looked at an accent, a filled danger and a quiet danger button on the dark Overview and light theme.
- It saw each at rest, on hover and on keyboard focus, zoomed in.
- The chamfer and 1px edge render; hover shows an edge; focus shows a thick edge on all three variants in both themes.

**Chip text that overflowed its cell** (compared with `main`, same window width of 1352 px):

- On `main`, the Persistence TRUST chip ("Signed" plus a team ID) was 132 px wide in a 100 px cell and was cut off silently at the cell edge.
- Before this fix the new chip was 145 px in the same cell, so it was also wider than its cell.
- Chips now stop at their cell with an ellipsis, and the Persistence trust chip and the History signal chip carry the full text in `title`.
- Measured on every page's chips afterwards: none is wider than its cell; only the 23 Persistence trust chips truncate, each with its full text in `title`.
- Team IDs are still cut to about "SIGNED JQ5...", because that column is 100 px wide. Widening it belongs to the table-style todo.

**Browser sweep** (2026-11-26, Chrome extension, the merged `main` built from this worktree and served on port 4961 with a scratch `HOME`):

- The window was 795 px wide and cannot be resized. I looked at screenshots of Overview, Apps with an app selected, Performance, Performance CPU and GPU, Network, Listeners, Privilege, Persistence, History and Guide, in dark and in light.
- I also loaded each page, in both themes, in a 400 px wide same-origin frame. The document scroll width was 400 on all 18 loads, so no page scrolls sideways. Tables scroll inside their card.
- Keyboard focus showed a thick edge, with the chamfer intact, on the Approve button in a real Overview row (light and dark), on the All chip on Network (light and dark) and on the Listeners sidebar item (light and dark).
- Hover showed an edge on the accent button in both themes. The quiet danger button filled on hover in both themes.
- Two defects were found and fixed. The severity chip on each Overview item shrank to "HI..." in the flex row; it now keeps its width. On Privilege, the Right text ran over the Requester text; both cells now wrap.
- After each fix I reloaded and looked again. No Overview tag is cut. No Privilege cell overflows.
- One defect is recorded, not fixed: the Network table's first columns shrink to about one character at 400 px, and the Apps and Listeners columns clip at 795 px. See the bug folder `network-table-columns-collapse-at-narrow-width`. It comes from the table grid, not the chips or buttons.
- The owner's session looked at the build with both fixes in a window 1352 px wide (not this agent). Dark: Overview, Apps with the inspector open beside the table, Performance overview tab, Network, Listeners, Privilege, Persistence and History looked right. Severity chips were whole and Privilege cells did not overlap.
- Light at 1352 px: Overview, Apps with the inspector open, Persistence and Guide (searched for "docker") looked right. Persistence trust chips truncate with an ellipsis as designed.
- Not looked at at 1352 px: the CPU and GPU tab; Network, Listeners, Privilege, History and Performance in light; keyboard focus and hover.
- With the Apps inspector open at 1352 px the table is squeezed. App names are cut ("Google Ch...", "Claude Co...", "identityse...") and the Listening and Signing columns clip. Added to the table bug record as an affected case.
- Every table carries a three-line export note, which is visually heavy. This is a design note for the page-shell and redesign todos, not a bug.
- Persistence still cuts team IDs in its Trust chip, as noted above; the full text is in `title`.

**Forced-colors and reduced-motion** (read in the CSS, not run):

- The browser extension cannot switch on forced colors or reduced motion, and DevTools emulation was not used, so neither was run.
- No `transition`, `animation` or `@keyframes` is declared anywhere in `web/index.html`; the words appear only in a comment and in the reduced-motion rule. No `scroll-behavior: smooth` is set.
- The forced-colors block names an edge and a fill for `.btn` and its `btn-acc`, `btn-danger` and quiet danger variants, and their disabled forms. It names only an edge, `border-color`, for `.chip`, `.tag`, `.badge` and `.sel-btn`; their fills are left to the browser. `.chip.on` also gets a Highlight outline.
- The tag and badge color variants (`ok`, `warn`, `bad`) share that border rule. Their meaning is in their text. No variant in the CSS is missed by name.
- Whether system colors in custom properties keep the button edge visible is unproven until T1c runs it.

**Not checked:**

- The 1352 px pages and states listed under the acceptance criteria, forced colors and reduced motion (T1c).
- A focused chip that is not selected, and a focused sidebar item on other pages.
- Hover on a filled danger button; only the quiet form was hovered.

**Deviations:**

- Chip colors were read as fill then text, so `#14382b on #7ee2b8` means fill `#14382b` and text `#7ee2b8`. The danger chip and every light-theme chip color are derived.
- `--warn` moved to `#f6c565`, `--bad` to `#ff8f7e`, and `--bg` and `--panel` to the canvas values.
- The repo has no JavaScript test harness, so no new test was added.
- Sidebar groups and the test that every `NAV` entry has a group belong to the page-shell todo, as this todo's Scope says.
