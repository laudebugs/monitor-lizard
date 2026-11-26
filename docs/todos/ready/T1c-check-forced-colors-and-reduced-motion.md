# T1c: Check forced colors, reduced motion and the wide layout on a real setup

| Field | Value |
|---|---|
| ID | `T1c` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | T1a |
| ADRs | None |
| Branch | `chore/t1c-check-forced-colors-and-reduced-motion` |
| PR title | `chore(todos): check forced colors and reduced motion on a real Mac` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Run the three checks the browser extension could not run for T1a: forced colors, reduced motion and the 1360 px layout. Fix any CSS defect that is small and local, and record the rest.

## Context

- The T1a Implementation Record lists what was read in the CSS and what was never run.
- The CSS has one `forced-colors` block and one `prefers-reduced-motion` block in `web/index.html`.

## Scope

**In scope:**

- Forced colors, reduced motion, and every page at 1360 px in both themes.
- Small CSS fixes for what you see, in new commits.

**Out of scope** (name the todo that covers it):

- Table layout at narrow widths: see `docs/bugs/network-table-columns-collapse-at-narrow-width/`.
- Sidebar groups and table styles: T1b.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Only if a check finds a defect |
| `docs/todos/ready/T1c-check-forced-colors-and-reduced-motion.md` | Implementation Record |

## Steps

- [ ] Build and serve: `cargo build`, then `HOME=<scratch folder> ./target/debug/monitor-lizard serve 4962`.
- [ ] Open System Settings, Accessibility, Display and turn on Increase contrast. Safari and Chrome on macOS apply it; forced colors itself is a Windows feature, so on a Mac this checks high-contrast rendering only. Say so in the record. If a Windows machine is available, turn on a high-contrast theme there and repeat.
- [ ] With it on, look at Overview (buttons and chips), Network (filter chips) and Apps with an app selected. Look at: every button has a visible edge; the focus edge on a Tab-focused button is thicker than the rest edge; chips keep a border and readable text; the selected chip is distinguishable from the others.
- [ ] Turn Increase contrast off. Turn on System Settings, Accessibility, Display, Reduce motion. Reload. Look at: nothing slides, fades or pulses on hover, focus, page change or the 2 s refresh. In DevTools, the Animations panel stays empty.
- [ ] Turn Reduce motion off. In a window at least 1360 px wide, look at every page in both themes. Look at: the inspector sits beside the table, the header stays put, no clipped text, no misaligned chip.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Increase contrast on | Buttons and chips keep an edge; text readable |
| Reduce motion on | No transition or animation runs |
| 1360 px, both themes | No horizontal scroll; the inspector is beside the table |

## Acceptance criteria

- Each of the three checks is recorded with what was seen, or marked as not possible with the reason.
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

- A Mac has no forced-colors mode. Increase contrast is a different mechanism, so a pass there does not prove the `forced-colors` block. The honest record may be "pending: needs a Windows machine".

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
