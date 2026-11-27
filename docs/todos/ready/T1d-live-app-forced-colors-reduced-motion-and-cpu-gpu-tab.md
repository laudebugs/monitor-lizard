# T1d: Live-app forced colors, reduced motion and the CPU and GPU tab

| Field | Value |
|---|---|
| ID | `T1d` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | T1c |
| ADRs | None |
| Branch | `chore/t1d-live-app-forced-colors-reduced-motion` |
| PR title | `chore(todos): check the live app under forced colors and reduced motion` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Run the part of T1c that needs the real app: forced colors and reduced motion on the live pages, and the CPU and GPU tab at 1352 px. This is the owner's Chrome DevTools check on a Mac.

## Context

- [T1c](../done/T1c-check-forced-colors-and-reduced-motion.md) did the emulated checks on the page's own markup and lists what it could not see.

## Scope

**In scope:**

- Overview, Apps with an app selected, Network and Guide under `forced-colors: active`: buttons, tags, badges, chips, select buttons, the focus edge, table rows and the Stop dialog.
- Hover, Tab, a page change and one 2 s refresh under `prefers-reduced-motion: reduce`; the Animations panel stays empty.
- The CPU and GPU tab at 1352 px in both themes.
- The optional macOS Increase contrast and Reduce motion look, recorded as not testing the `forced-colors` block.
- Small CSS fixes for what you see, in new commits.

**Out of scope:** table layout at narrow widths, see `docs/bugs/network-table-columns-collapse-at-narrow-width/`.

## Target files

| File | Change |
|---|---|
| `web/app.css` (`web/index.html` before T2 merges) | Only if a check finds a defect |
| `docs/todos/ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md` | Implementation Record |

## Steps

- [ ] Build and serve: `cargo build`, then `HOME=<scratch folder> ./target/debug/monitor-lizard serve 4962`. Open it in Chrome.
- [ ] DevTools, Rendering panel: set forced-colors to `active`. Look at the four pages and each button variant, enabled and disabled.
- [ ] Set forced-colors back, set prefers-reduced-motion to `reduce`, then hover, Tab, change page and wait through one refresh.
- [ ] In a window at least 1352 px wide, look at the CPU and GPU tab in both themes.
- [ ] Optional: Increase contrast, then Reduce motion, in System Settings.

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

Plan table row.

## Risks and open questions

- DevTools emulation uses Chrome's forced-colors palette, which differs from Windows High Contrast themes. Record the result as emulated.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
