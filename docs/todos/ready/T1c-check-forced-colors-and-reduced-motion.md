# T1c: Check forced colors and reduced motion

| Field | Value |
|---|---|
| ID | `T1c` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | T1a |
| ADRs | None |
| Branch | `chore/t1c-check-forced-colors-and-reduced-motion` |
| PR title | `chore(todos): check forced colors and reduced motion` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Run the checks T1a did not cover: `forced-colors: active` and `prefers-reduced-motion: reduce`. Chrome DevTools emulates both media features on a Mac, and this is the owner's DevTools check. Fix any CSS defect that is small and local, and record the rest. The 1352 px page sweep is done: the page-shell todo records all nine pages in both themes.

## Context

- The T1a Implementation Record lists what was read in the CSS and what was never run.
- The CSS has one `forced-colors` block and one `prefers-reduced-motion` block in `web/index.html`.
- macOS Increase contrast does not switch on `forced-colors`, so it does not exercise that block. It is only a second look.

## Scope

**In scope:**

- Forced colors and reduced motion through DevTools emulation.
- The CPU and GPU tab at 1352 px in both themes, the one 1352 px state the page-shell sweep did not list.
- Small CSS fixes for what you see, in new commits.

**Out of scope** (name the todo that covers it):

- Table layout at narrow widths: see `docs/bugs/network-table-columns-collapse-at-narrow-width/`.
- Sidebar groups, table styles and the 1352 px sweep of the nine pages: T1b (done).

## Target files

| File | Change |
|---|---|
| `web/index.html` | Only if a check finds a defect |
| `docs/todos/ready/T1c-check-forced-colors-and-reduced-motion.md` | Implementation Record |

## Steps

- [ ] Build and serve: `cargo build`, then `HOME=<scratch folder> ./target/debug/monitor-lizard serve 4962`. Open it in Chrome.
- [ ] Open DevTools, then the Rendering panel. Set "Emulate CSS media feature forced-colors" to `active`.
- [ ] Look at Overview, Apps with an app selected, Network and Guide. Look at each button variant, enabled and disabled: plain, accent, danger and quiet danger. Look at tags, badges, the filter chips with one selected, the select buttons, the focus edge after Tab, and the sidebar. Every button has a visible edge; focus is thicker than rest; chips keep a border and readable text; the selected chip stands out.
- [ ] Set forced-colors back to no emulation. Set "Emulate CSS media feature prefers-reduced-motion" to `reduce`. Hover, Tab, change page and wait through one 2 s refresh. Nothing slides, fades or pulses; the Animations panel stays empty.
- [ ] Optional second look: in System Settings, Accessibility, Display, turn on Increase contrast, then Reduce motion, and reload. Say in the record that this does not test the `forced-colors` block.
- [ ] In a window at least 1352 px wide, look at the CPU and GPU tab in both themes.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `forced-colors: active` | Buttons and chips keep an edge; text readable; disabled buttons look disabled |
| `prefers-reduced-motion: reduce` | No transition or animation runs |
| CPU and GPU tab, 1352 px, both themes | No horizontal scroll; no text clipped |

## Acceptance criteria

- Each check is recorded with what was seen, or marked as not possible with the reason.
- Any defect is fixed in a small commit or has a folder in `docs/bugs/`.

## Tests

None: these are manual checks.

## Verification

```sh
make check
```

## Docs to update

The T1a Implementation Record gets a one-line pointer to this todo's result; plan table row.

## Risks and open questions

- DevTools emulation uses Chrome's forced-colors palette, which differs from Windows High Contrast themes. A pass there is strong evidence, but a Windows run would be the final proof. Record it as emulated.
- The extension may not reach the Rendering panel. If so, the owner runs these steps by hand.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
