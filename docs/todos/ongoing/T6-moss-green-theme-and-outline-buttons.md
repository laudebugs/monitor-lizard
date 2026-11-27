# T6: Moss green theme and outline buttons

| Field | Value |
|---|---|
| ID | `T6` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 and D16 |
| Depends on | T1a, T1b |
| ADRs | None |
| Branch | `feat/moss-theme-outline-buttons` |
| PR title | `feat(web): switch to the Moss green theme with outline buttons` |
| Size | M (200-400) |
| PR | [#90](https://github.com/laudebugs/monitor-lizard/pull/90) |

## Goal

Replace the violet console palette with the Moss green palette the owner chose on 2026-11-27 (design canvas boards J, K, L and M), in both themes, and replace the chamfered buttons with square outline buttons (option B). Green becomes the brand and selection colour, so the "OK" status moves to cyan: a healthy chip must never look like a selected one.

## Context

- [T1a](../done/T1a-design-tokens-buttons-chips.md) made the tokens and the chamfered buttons; [T1c](../done/T1c-check-forced-colors-and-reduced-motion.md) checked forced colors on them. This todo changes the values and the button shape, not the token names.
- The owner's decision: Moss palette, option B (outline) buttons, boards J, K, L and M of the canvas "Sys Monitor Redesign Directions".
- The logo and favicon are not touched here: the pulse-and-tail logo is [L2](L2-pulse-and-tail-logo-and-app-icons.md) (PR 88), which also adds a forced-colors `.logo` rule to `web/app.css`.

## Scope

**In scope:**

- Both token blocks in `web/app.css` (dark and `:root[data-theme=light]`), with the Moss values.
- New tokens the palette needs and the app lacked: `--edge`, `--edge2`, `--line2`, `--fg2`, `--acchov`, `--link`, `--input-bg`, `--input-bd`, and `--s2` (the second chart series). `--cut` goes: nothing clips any more.
- The `.btn` family, chips, filters, fields, links, focus rings, the second chart series and the capacity bar.
- A Node test that reads `web/app.css` and holds the contrast ratios.

**Out of scope:**

- The logo, favicon and app icons ([L2](L2-pulse-and-tail-logo-and-app-icons.md)).
- Checking forced colors in a forced-colors browser ([T1d](../ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md)).
- The exported Storage report, a standalone print page with its own inline style.

## Target files

| File | Change |
|---|---|
| `web/app.css` | Moss tokens; outline buttons; chips, fields and focus on the new tokens; forced-colors block for the new buttons |
| `web/page-performance.js` | Second chart series uses `--s2`; the purgeable capacity segment is hatched |
| `tests/web/theme-contrast.test.mjs` | (new) contrast ratios, no violet, no stray hex, button rules, forced-colors rules |
| `docs/plans/storage-and-guide.md` | Decision D16 and the T6 row |
| `docs/agents/layout.md` | Palette and token note |

## Steps

- [ ] Rewrite the dark and light token blocks.
- [ ] Rewrite `.btn`, its variants, hover, focus, disabled and the forced-colors block.
- [ ] Move chips, fields, selects and borders to `--edge2`, `--input-*` and `--line2`.
- [ ] Move chart series, legends and meters that meant "OK" to cyan or `--s2`; keep brand and selection on `--acc`.
- [ ] Add `tests/web/theme-contrast.test.mjs`.
- [ ] Check every page in dark and light in a real browser.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `aria-disabled=true` or `disabled` on any button variant | Dashed `--mut` border, `--mut` label, no hover change, `not-allowed` cursor |
| Forced colors active | Every button keeps a visible border; a disabled one is GrayText |
| Green and cyan in one chart | Series differ in lightness (`--s2` is darker than `--acc`) and each has a text legend |
| Used and purgeable capacity segments | Purgeable is hatched, so it differs without relying on hue |

## Acceptance criteria

- No violet hex remains in `web/` except the favicon, which L2 replaces.
- Every pair in the contrast table (Implementation Record) meets its threshold, and a test holds it.
- The pages named in Verification look right in both themes.

## Tests

`tests/web/theme-contrast.test.mjs`: 14 cases (contrast per theme, disabled readability, hue separation, token parity, every `var(--x)` defined, no old colours, no hex outside the token blocks, button rules, forced-colors rules).

## Verification

```sh
make check
make test
make check-docs
node --test tests/web/theme-contrast.test.mjs
```

## Docs to update

`docs/plans/storage-and-guide.md` (D16, T6 row), `docs/agents/layout.md`.

## Risks and open questions

- Forced colors are checked by reading the rules only (T1d does the live check).
- Whether the owner wants the danger button's hover to differ from the plain one is not on the boards; it keeps a solid fill with a `--fg` border.

## Definition of done

- [x] Claimed: first commit moved this file into `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages (pending: the owner's review step)
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

**Built.** The Moss palette in both token blocks of `web/app.css`, with the owner's values. Outline buttons replace the chamfered ones: no `clip-path`, a 2px border in `--edge`, transparent fill, mono caps 600 12px. Primary is solid `--acc` (hover `--acchov`); danger is solid `--bad`; quiet danger keeps a `--bad` border and fills on hover. Plain hover is `--sel` with an `--fg` border. Focus is a 3px `--fg` outline at a 2px offset. A disabled or `aria-disabled` button has a dashed `--mut` border, a `--mut` label, 75% opacity, no hover change and the `not-allowed` cursor; the class names are unchanged, so no script changed for buttons. Chips, selects, the search field, the copy box and links use `--edge2`, `--input-bd`, `--input-bg`, `--line2` and `--link`. Every `--ok` use that meant "healthy" is cyan now (the OK chip, the Overview tile value). `--cut` is gone.

**Changed beyond the token swap, and why.**

- New token `--s2` (dark `#3aa6c4`, light `#1a8fa9`) for the second chart series: Write, System, Upload, Wired and Tiler used `--ok`. Green and cyan are the same lightness, so the two lines would differ by hue alone. `--s2` is darker (ratio to `--acc` is above 1.3, held by a test) and still 3:1 on every surface.
- The purgeable segment of the capacity bar is hatched (`web/page-performance.js`). Green and amber also sit at near-equal lightness, and green against amber is a hard pair for some readers; stripes carry the difference in both themes, in the bar and in its legend square.
- Global focus ring is `--fg` (was `--acc`), so it shows on a green primary button and on selected rows.
- Tokens added because the app lacked them: `--edge`, `--edge2`, `--line2`, `--fg2`, `--acchov`, `--link`, `--input-bg`, `--input-bd`. The `--edge` custom property used to be a button-local variable; the buttons now use `--bd`, so the global name is free.

**Values I adjusted from the brief, after the contrast script failed them.**

| Token | Brief | Now | Why |
|---|---|---|---|
| light `--acc` | `#17803f` | `#157a3a` | Text in `--acc` on `--panel2` and `--sel` was 4.29 and 4.25; now 4.64 and 4.60 |
| light `--edge2` | `#6f9a86` | `#5f8a73` | 2.92 on `--bg` and 2.71 on `--panel2`; now 3.61 and 3.35 |
| dark `--edge2` | `#517363` | `#56786a` | 3.00 on `--panel2` was on the line; now 3.23 |

The check is `tests/web/theme-contrast.test.mjs`; the table below is its pairs, recomputed from the stylesheet.

| Pair (foreground on background) | Needs | Dark | Light |
|---|---|---|---|
| `--fg` on `--bg` | 4.5 | 16.69 | 15.58 |
| `--fg` on `--panel` | 4.5 | 15.75 | 16.89 |
| `--fg` on `--sel` | 4.5 | 12.55 | 14.36 |
| `--fg2` on `--panel` | 4.5 | 12.41 | 9.69 |
| `--mut` on `--bg` | 4.5 | 9.48 | 6.03 |
| `--mut` on `--panel` | 4.5 | 8.95 | 6.53 |
| `--mut` on `--panel2` | 4.5 | 8.00 | 5.60 |
| `--mut` on `--sel` | 4.5 | 7.13 | 5.56 |
| `--acc` on `--bg` | 4.5 | 9.65 | 5.00 |
| `--acc` on `--panel` | 4.5 | 9.11 | 5.42 |
| `--acc` on `--panel2` | 4.5 | 8.15 | 4.64 |
| `--acc` on `--sel` | 4.5 | 7.25 | 4.60 |
| `--accfg` on `--acc` | 4.5 | 9.71 | 5.42 |
| `--accfg` on `--acchov` | 4.5 | 12.30 | 6.71 |
| `--link` on `--panel` | 4.5 | 11.54 | 5.42 |
| `--ok` on `--panel` | 4.5 | 10.43 | 5.36 |
| `--warn` on `--panel` | 4.5 | 11.01 | 5.93 |
| `--bad` on `--panel` | 4.5 | 7.98 | 6.57 |
| `--badfg` on `--bad` | 4.5 | 8.29 | 6.57 |
| `--fg` on `--input-bg` | 4.5 | 17.18 | 16.89 |
| `--mut` on `--input-bg` | 4.5 | 9.76 | 6.53 |
| `--input-bd` on `--input-bg` | 3 | 3.72 | 3.26 |
| `--edge` on `--panel` | 3 | 5.59 | 4.91 |
| `--edge` on `--panel2` | 3 | 5.00 | 4.21 |
| `--edge2` on `--bg` | 3 | 3.82 | 3.61 |
| `--edge2` on `--panel` | 3 | 3.61 | 3.91 |
| `--edge2` on `--panel2` | 3 | 3.23 | 3.35 |
| `--s2` on `--panel` | 3 | 6.25 | 3.79 |
| `--s2` on `--panel2` | 3 | 5.59 | 3.25 |
| `--chip-ok-fg` on `--chip-ok-bg` | 4.5 | 8.31 | 6.77 |
| `--chip-warn-fg` on `--chip-warn-bg` | 4.5 | 8.15 | 6.94 |
| `--chip-bad-fg` on `--chip-bad-bg` | 4.5 | 8.16 | 6.94 |
| `--chip-n-fg` on `--chip-n-bg` | 4.5 | 10.52 | 7.98 |

A disabled button (label and dashed border at 75% opacity) reads 5.57 on `--panel` in dark. In light it reads 3.66 on `--panel` and 3.33 on `--panel2`: below 4.5, above the 3.0 the test requires. Disabled controls are exempt from the 4.5 rule, and the dashed border also marks the state, so `--mut` stays as the owner chose it.

**Verified in a real browser** (Chrome, scratch server on a free port, scratch `HOME`, no window resize), dark and light by setting `data-theme`: Overview, Apps, Network, Persistence (empty on this Mac), Privilege (a faked sudo capture: requester detail with enabled and disabled Freeze, Resume, Stop and Force stop, and the Freeze dialog), Performance (all four tabs I opened: overview tiles, CPU and GPU, Disk and Network with the hatched capacity bar, Memory), Storage (faked scan: status strip, treemap, clean-ups list, the "Move to the Trash" dialog with a danger button and a refused row), Guide, and the global search menu. Primary buttons are solid green and the rest are outlined. Chips are cyan for OK, amber for review and coral for bad, so a selected chip (green border, `--sel` fill) never reads as a healthy one. Tab focus showed a white 2px ring on a chip and a 3px ring on a dialog button. A disabled Rescan stayed dashed under the pointer.

**Not verified.**

- Forced colors: read the rules only, in a browser without forced colors. The block names `ButtonText`, `Highlight` and `GrayText` for every variant, but a forced-colors browser has not drawn it ([T1d](../ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md)). In forced colors the primary and plain buttons look alike, as before.
- `prefers-color-scheme`: the stylesheet has no media query for it; the bootstrap script and `applyTheme` in `core.js` copy the setting into `data-theme`, which I set directly. The OS-level switch was not toggled.
- Pages I did not open in the browser: Listeners and History (same shared rules), and the Trash and Freeze dialogs in their final "done" state.
- The Storage, Trash and Freeze screens ran on scripted replies (the server refuses to scan a scratch home folder).
- The favicon still uses the old violet; L2 (PR 88) replaces the logo and icons.
