# T1c: Check forced colors and reduced motion

| Field | Value |
|---|---|
| ID | `T1c` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | T1a |
| ADRs | None |
| Branch | `chore/t1c-check-forced-colors-and-reduced-motion` |
| PR title | `fix(web): check forced colors and reduced motion, and fix the aria-disabled edge` |
| Size | S (under 200 changed lines) |
| PR | — |

## Goal

Run the checks T1a did not cover: `forced-colors: active` and `prefers-reduced-motion: reduce`. This todo covers the part that runs without the app: headless Chromium emulation (Playwright) of both media features against the page's own markup. The checks that need the live app on a Mac moved to [T1d](../ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md). Fix any CSS defect that is small and local, and record the rest. The 1352 px page sweep is done: the page-shell todo records all nine pages in both themes.

## Context

- The T1a Implementation Record lists what was read in the CSS and what was never run.
- The CSS has one `forced-colors` block and one `prefers-reduced-motion` block in `web/index.html`.
- macOS Increase contrast does not switch on `forced-colors`, so it does not exercise that block. It is only a second look.

## Scope

**In scope:**

- Forced colors and reduced motion by emulation, on the page's button, chip, tag and select markup.
- Small CSS fixes for what you see, in new commits.

**Out of scope** (name the todo that covers it):

- Live-app pages under forced colors, reduced motion on the live app, and the CPU and GPU tab at 1352 px in both themes: T1d.
- Table layout at narrow widths: see `docs/bugs/network-table-columns-collapse-at-narrow-width/`.
- Sidebar groups, table styles and the 1352 px sweep of the nine pages: T1b (done).

## Target files

| File | Change |
|---|---|
| `web/index.html` | Only if a check finds a defect |
| `docs/todos/done/T1c-check-forced-colors-and-reduced-motion.md` | Implementation Record |

## Steps
- [x] (Method changed: the server is macOS-only, so the page was served as a static file and its own markup inserted; see the record.)
- [x] Forced colors emulated: each button variant enabled and disabled, tags, badges, chips with one selected, the select button, the focus edge after Tab, and the sidebar. Result in the record.
- [x] Reduced motion emulated: no animation or transition runs. The page declares none.
- [ ] Not done here, moved to T1d: Overview, Apps, Network and Guide with real data; hover, Tab, page change and a 2 s refresh on the live app.
- [ ] Not done here, moved to T1d: the optional macOS Increase contrast and Reduce motion look, and the CPU and GPU tab at 1352 px in both themes.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `forced-colors: active` | Buttons and chips keep an edge; text readable; disabled buttons look disabled |
| `prefers-reduced-motion: reduce` | No transition or animation runs |
| CPU and GPU tab, 1352 px, both themes | Moved to T1d |

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` green; `make test` not run here (the Rust needs macOS, and this change touches no Rust)
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-27)

Headless Chromium on Linux ran these checks through Playwright. The owner's Mac Chrome run is still owed, and is now T1d.

**Method.** The server is macOS-only, so the app could not run. I served `web/index.html` as a static file with every `/api/*` call answering 404, so the page shows only its shell. I then inserted the page's own markup into it: each `.btn` variant enabled, with `disabled` and with `aria-disabled`, `.chip` selected and not, `.tag` and `.badge` variants and a `.sel-btn`. I Tabbed to a button. Playwright's `forcedColors: 'active'` and `reducedMotion: 'reduce'` set the media features. This is Chromium's emulated palette, not Windows High Contrast.

**Forced colors: pass, after one fix.**

- Every button variant has a visible edge, and a disabled button is distinguishable from an enabled one.
- Defect found and fixed: a button with `aria-disabled=true` and a variant class (the Stop dialog's quiet danger button is one) took the enabled edge and ink, because the variant rule outranks the `aria-disabled` rule. Only `opacity` marked it disabled. The `GrayText` rule now lists the `aria-disabled` selectors beside the `[disabled]` ones. Chromium paints its emulated `GrayText` dark red, so a Windows theme will look different.
- The focused button (quiet danger) has a thicker `Highlight` edge than the rest-state edge of the others.
- The selected chip has a thick `Highlight` edge and the other chips keep a `ButtonText` border. Tags, badges and the select button keep a border and readable text.

**Reduced motion: pass, by construction.** `web/index.html` has no `transition`, `animation`, `@keyframes`, `.animate(` or smooth-scroll rule. The reduced-motion block guards the first one added. With `reduce` set, `document.getAnimations()` was empty and no element in the probe had a non-zero transition duration. `requestAnimationFrame` is used twice (table decoration on resize, drawer drag) and moves nothing by itself.

**Moved to T1d (needs the live app on a Mac):** the four pages with real data under forced colors, hover, page change and a 2 s refresh under reduced motion, the CPU and GPU tab at 1352 px in both themes, and the optional macOS Increase contrast and Reduce motion look.
