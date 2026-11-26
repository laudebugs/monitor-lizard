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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
