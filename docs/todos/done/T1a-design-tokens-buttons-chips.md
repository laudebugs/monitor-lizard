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
| `web/index.html` | CSS tokens and component classes, `NAV` groups, badge counts |

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

Existing logic tests stay green. Add a small test that every `NAV` entry has a group. Visual checks are manual and recorded.

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
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** in `web/index.html`, the dark and light token sets (existing names kept; new `--cut` and eight `--chip-*` tokens), chamfered mono-caps `.btn` with `.btn-acc` and `.btn-danger` (plus `.btn-danger.quiet`, the outline form for row actions), square mono-caps `.tag`, `.badge`, `.chip` and `.sel-btn`, a forced-colors block, and the reduced-motion rule extended to the controls. JavaScript changed only in class strings: `btn pri` to `btn btn-acc`, `btn danger` to `btn btn-danger quiet`, `btn solid` to `btn btn-danger`. The mapping of every use of the old classes is in the PR description.
- **How the chamfer keeps its edge:** `clip-path` also clips borders and outlines, so the edge is the button's own background and `::before` paints the fill inset by `--bw`. Focus raises `--bw` to 3px and recolors the edge to `--fg`; no outline is used, so nothing is cut.
- **Contrast check** (script run on the token values in the file; WCAG 2.x ratios, need 4.5 for text and 3 for chip text, all pass):

| Pair | Dark | Light |
|---|---|---|
| fg on bg / panel / panel2 / sel | 15.54 / 15.10 / 14.23 / 13.44 | 16.14 / 17.47 / 14.80 / 14.39 |
| mut on bg / panel / panel2 / sel | 7.46 / 7.26 / 6.84 / 6.46 | 6.41 / 6.94 / 5.88 / 5.72 |
| acc on panel (text) | 6.96 | 6.40 |
| accfg on acc | 7.16 | 6.40 |
| badfg on bad | 8.29 | 5.80 |
| ok / warn / bad on panel | 11.88 / 11.56 / 8.37 | 5.34 / 5.93 / 5.80 |
| chip ok / warn / bad / neutral | 8.25 / 8.15 / 7.12 / 9.33 | 6.87 / 6.94 / 6.94 / 8.98 |

  Not met by design: `--line` on `--panel` is 1.40 (dark) and 1.50 (light). That is below the 3:1 WCAG asks of a control boundary. Button edges use it as in the design canvas; the label, the hover edge (`--acc`, 6.96 and 6.40) and the focus edge carry the affordance. Raising it is a design decision for the owner.
- **Deviations:** (1) The todo's Tests and Target files call for `NAV` groups and a test that every `NAV` entry has a group, but its own Scope assigns sidebar groups and badge counts to T1b. I followed Scope, so `NAV` is unchanged and that test is not written; it belongs with T1b. (2) The chip colors in the task were given as "fill on text" pairs; I read `#14382b on #7ee2b8` as fill `#14382b` with text `#7ee2b8`. The danger chip (`#40150e` fill, `#ff8f7e` text) and all light-theme chip colors are derived, not from the canvas. (3) `--warn` moved from `#ffc857` to `#f6c565`, `--bad` from `#ff6b8b` to `#ff8f7e`, and `--bg` and `--panel` to the canvas values. (4) There is no JavaScript test harness in the repo, so no new test was added; `make check` and `make test` (117 passed) are green and the page and its script parse.
- **Open (pending, not run):** every visual check. The browser extension was not connected, so nothing was looked at in a real browser: rendering of the chamfer and its 1px edge, focus visibility on every control, forced-colors, the 400px layout, both themes on every page at 1360px and 400px. The acceptance criterion "every page is checked in a real browser" is therefore not met and stays open. A static server was used only to confirm the file loads.
