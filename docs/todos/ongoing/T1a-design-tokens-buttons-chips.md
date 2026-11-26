# T1a: Design tokens, buttons and chips

| Field | Value |
|---|---|
| ID | `T1a` |
| Status | `ongoing` |
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

- No page shows the old rounded or flat buttons.
- A contrast check of tokens passes; the result is recorded in the todo.
- Every page is checked in a real browser in both themes at 1360 px and 400 px.

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
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26. Not finished: the browser checks below are open, so this todo stays in `ongoing/`.

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

**Not looked at (open):**

- The other pages in both themes at 1352 px and 400 px
- Forced-colors mode
- A focused chamfered button inside a real page, as opposed to a probe row
- `prefers-reduced-motion`

**Deviations:**

- Chip colors were read as fill then text, so `#14382b on #7ee2b8` means fill `#14382b` and text `#7ee2b8`. The danger chip and every light-theme chip color are derived.
- `--warn` moved to `#f6c565`, `--bad` to `#ff8f7e`, and `--bg` and `--panel` to the canvas values.
- The repo has no JavaScript test harness, so no new test was added.
- Sidebar groups and the test that every `NAV` entry has a group belong to the page-shell todo, as this todo's Scope says.
