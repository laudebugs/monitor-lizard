# T6: Moss green theme and outline buttons

| Field | Value |
|---|---|
| ID | `T6` |
| Status | `done` |
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
- The logo and favicon are not touched here: the pulse-and-tail logo is L2 (PR 88), which also adds a forced-colors `.logo` rule to `web/app.css`.

## Scope

**In scope:**

- Both token blocks in `web/app.css` (dark and `:root[data-theme=light]`), with the Moss values.
- New tokens the palette needs and the app lacked: `--edge`, `--edge2`, `--line2`, `--fg2`, `--acchov`, `--link`, `--input-bg`, `--input-bd`, and `--s2` and `--s3` (the second and third chart series). `--cut` goes: nothing clips any more.
- The `.btn` family, chips, filters, fields, links, focus rings, the second chart series and the capacity bar.
- A Node test that reads `web/app.css` and holds the contrast ratios.

**Out of scope:**

- The logo, favicon and app icons (L2, PR 88).
- Checking forced colors in a forced-colors browser ([T1d](../ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md)).
- The exported Storage report, a standalone print page with its own inline style.

## Target files

| File | Change |
|---|---|
| `web/app.css` | Moss tokens; outline buttons; chips, fields and focus on the new tokens; forced-colors block for the new buttons |
| `web/page-performance.js` | Second and third chart series use `--s2` and `--s3`; the purgeable capacity segment is hatched; legend squares carry a class |
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
| Green, cyan and amber in one chart | Series differ pairwise in lightness (at least 1.3:1; `--acc` against `--s2` at least 1.5:1) and each has a text legend |
| Used and purgeable capacity segments | Purgeable is hatched, so it differs without relying on hue |

## Acceptance criteria

- No violet hex remains in `web/` except the favicon, which L2 replaces.
- Every pair in the contrast table (Implementation Record) meets its threshold, and a test holds it.
- The pages named in Verification look right in both themes.

## Tests

`tests/web/theme-contrast.test.mjs`: 20 cases (per theme: contrast, disabled readability, hue separation, lightness steps between series, `--ok` has a consumer; then token parity, every `var(--x)` defined, no old colours, no literal colour outside the token blocks, button rules, forced-colors rules).

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
- [x] `/code-review` run per `AGENTS.md`: two independent reviews (standards and spec) found no blockers; the fixes are in the second feature commit (pending: the owner's review step)
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27, updated after review the same day.

**Built.** The Moss palette in both token blocks of `web/app.css`. Outline buttons replace the chamfered ones: no `clip-path`, a 2px border in `--edge`, transparent fill, mono caps 600 12px. Primary is solid `--acc` (hover `--acchov`); danger is solid `--bad` (hover keeps the fill and takes an `--fg` border); quiet danger keeps a `--bad` border and fills on hover. Plain hover is `--sel` with an `--fg` border. Focus is a 3px `--fg` outline at a 2px offset. A disabled or `aria-disabled` button has a dashed `--mut` border, a `--mut` label, 75% opacity, no hover change and the `not-allowed` cursor. Class names are unchanged, so no script changed for buttons. `--cut` is gone; the old button-local `--edge` is now `--bd`, which frees `--edge` as a global token.

**How `--ok` is consumed.** The stylesheet does not read `--ok`. The cyan Overview tile value reaches it through `'color:var(--' + col + ')'` in `web/page-overview.js`, and the "Needs review" tile passes `'ok'` when the count is zero. The OK chip uses `--chip-ok-bg` and `--chip-ok-fg`, not `--ok`. A test asserts `--ok` is defined in both themes and that the tile builder still reads it this way, so a rename fails.

**Changed beyond the token swap, and why.**

- `--s2` (dark `#35a0c0`, light `#1f9fbd`) and `--s3` (dark `#ffd37a`, light `#ac7200`) are the second and third chart series. Write, System, Upload, Wired and Tiler used `--ok`, now cyan, which is the same lightness as the green accent. Compressed memory used `--warn`, which sits at nearly the accent's lightness in both themes. Each series on one chart now differs from the others by at least 1.3:1 (accent against `--s2` by at least 1.5:1). A test lists the series per chart, and every hue keeps its meaning (green, cyan, amber). `--warn` stays the text and meter colour.
- The purgeable capacity segment and its legend square are hatched `--s3` stripes, so green and amber do not rely on hue alone.
- The global focus ring is `--fg` (was `--acc`) for buttons, inputs, textareas and links. On a green primary button an accent ring would vanish, and on a selected row it would blend into the green selection bar. The canvas draws the input focus in `--acc`; I did not follow it for that reason, and it is the owner's call.
- `.item p` text uses `--fg2` (was `--mut`). `.item` and table-row separators use `--line2` (was `--line`). A new global `a{color:var(--link)}` replaces the browser's default link colour. Chips, `.rowcopy`, `.navtoggle` and the select button borders use `--edge2`; the search field and the copy box use `--input-bg` and `--input-bd`.
- Tokens added because the app lacked them: `--edge`, `--edge2`, `--line2`, `--fg2`, `--acchov`, `--link`, `--input-bg`, `--input-bd`, `--s2`, `--s3`.
- Forced colors: the capacity bar segments keep their colours and get a `ButtonText` divider (`forced-color-adjust:none`), and the legend squares get a `ButtonText` border, so used, purgeable and free stay apart. Chips, tags and badges already name `ButtonText` borders in the forced-colors block, and they have a real 1px border. This comes from reading the rules; no forced-colors browser drew it.
- The exported Storage report (`REPORT_CSS` in `web/page-storage-logic.js`) is a standalone print page with its own neutral inline colours. It is not themed and has no hatching.

**Values changed from the brief, after the contrast script failed them.** Columns of ratios are bg, panel, panel2 and sel.

| Token | Brief | Now | Before | After |
|---|---|---|---|---|
| light `--acc` | `#17803f` | `#157a3a` | 4.62 / 5.00 / 4.29 / 4.25 | 5.00 / 5.42 / 4.64 / 4.60 |
| light `--edge2` | `#6f9a86` | `#5f8a73` | 2.92 / 3.16 / 2.71 / 2.69 | 3.61 / 3.91 / 3.35 / 3.32 |
| dark `--edge2` | `#517363` | `#5a7d6e` | 3.55 / 3.35 / 3.00 / 2.67 | 4.09 / 3.86 / 3.46 / 3.08 |

Light `--link` follows `--acc` (`#157a3a`; the brief said `#17803f`). Light `--input-bd` is `#6f9784` as the brief gave it (3.26 on `--input-bg`); I could not compare it with the canvas board.

The test pairs, recomputed from the stylesheet:

| Pair | Needs | Dark | Light |
|---|---|---|---|
| `--fg` / `--bg` | 4.5 | 16.69 | 15.58 |
| `--fg` / `--panel` | 4.5 | 15.75 | 16.89 |
| `--fg` / `--sel` | 4.5 | 12.55 | 14.36 |
| `--fg2` / `--panel` | 4.5 | 12.41 | 9.69 |
| `--mut` / `--bg` | 4.5 | 9.48 | 6.03 |
| `--mut` / `--panel` | 4.5 | 8.95 | 6.53 |
| `--mut` / `--panel2` | 4.5 | 8.00 | 5.60 |
| `--mut` / `--sel` | 4.5 | 7.13 | 5.56 |
| `--acc` / `--bg` | 4.5 | 9.65 | 5.00 |
| `--acc` / `--panel` | 4.5 | 9.11 | 5.42 |
| `--acc` / `--panel2` | 4.5 | 8.15 | 4.64 |
| `--acc` / `--sel` | 4.5 | 7.25 | 4.60 |
| `--accfg` / `--acc` | 4.5 | 9.71 | 5.42 |
| `--accfg` / `--acchov` | 4.5 | 12.30 | 6.71 |
| `--link` / `--panel` | 4.5 | 11.54 | 5.42 |
| `--ok` / `--panel` | 4.5 | 10.43 | 5.36 |
| `--warn` / `--panel` | 4.5 | 11.01 | 5.93 |
| `--bad` / `--panel` | 4.5 | 7.98 | 6.57 |
| `--badfg` / `--bad` | 4.5 | 8.29 | 6.57 |
| `--fg` / `--input-bg` | 4.5 | 17.18 | 16.89 |
| `--mut` / `--input-bg` | 4.5 | 9.76 | 6.53 |
| `--input-bd` / `--input-bg` | 3 | 3.72 | 3.26 |
| `--edge` / `--panel` | 3 | 5.59 | 4.91 |
| `--edge` / `--panel2` | 3 | 5.00 | 4.21 |
| `--edge2` / `--bg` | 3 | 4.09 | 3.61 |
| `--edge2` / `--panel` | 3 | 3.86 | 3.91 |
| `--edge2` / `--panel2` | 3 | 3.46 | 3.35 |
| `--edge2` / `--sel` | 3 | 3.08 | 3.32 |
| `--s2` / `--panel` | 3 | 5.84 | 3.11 |
| `--s3` / `--panel` | 3 | 12.49 | 4.07 |
| `--chip-ok-fg` / `--chip-ok-bg` | 4.5 | 8.31 | 6.77 |
| `--chip-warn-fg` / `--chip-warn-bg` | 4.5 | 8.15 | 6.94 |
| `--chip-bad-fg` / `--chip-bad-bg` | 4.5 | 8.16 | 6.94 |
| `--chip-n-fg` / `--chip-n-bg` | 4.5 | 10.52 | 7.98 |
| `--acc` / `--s2` | 1.5 | 1.56 | 1.74 |
| `--acc` / `--s3` | 1.3 | 1.37 | 1.33 |
| `--s2` / `--s3` | 1.3 | 2.14 | 1.31 |

A disabled button (label and dashed border at 75% opacity) reads 5.57 on `--panel` in dark. In light it reads 3.66 on `--panel` and 3.33 on `--panel2`: below 4.5, above the 3.0 the test requires. Disabled controls are exempt from the 4.5 rule and the dashed border also marks the state, so `--mut` stays as the owner chose it.

**No second cue on the memory chart.** The stacked areas are 0.25-opacity fills in a fixed order (App at the bottom, then Wired, Compressed, Cached), and the legend gives each layer's value in text. A pattern fill would need an SVG pattern per layer. Lightness steps, order and text carry the meaning, so I recorded this as a choice, not a gap.

**Verified in a real browser** (Chrome, scratch server on a free port, scratch `HOME`, no window resize), dark and light by setting `data-theme`.

- Overview, Apps, Network, Performance (overview, CPU and GPU, Disk and Network, Memory), Guide, the global search menu and Listeners ran on this Mac's live data.
- Persistence, History, Privilege and Storage ran on scripted `fetch` replies, because the server refuses to scan a scratch home folder and a scratch Mac has no startup items, history or sudo capture.
- Persistence (three faked items): "Added" in amber and "Changed" in neutral, "New" and "Unsigned" chips, outlined "Mark expected" buttons, and the selected filter chip filled, in both themes.
- History (faked hosts and actions): tables with outlined export buttons and signal chips, and soft but visible row dividers, in both themes.
- Listeners: amber "Exposed" chips, filter chips, the protocol select and outlined buttons, in both themes.
- Privilege: the requester detail with enabled and dashed-disabled Freeze, Resume, Stop and Force stop, and the Freeze dialog with its solid danger button.
- Storage: status strip, hatched capacity bar, treemap, clean-ups list, the Trash dialog with a refused row, and a dashed disabled Rescan.
- Memory chart in light and dark after the `--s3` change: the green, cyan and amber layers differ visibly in lightness.
- Tab focus showed a white 2px ring on a chip and a 3px ring on a dialog button.

**Not verified.**

- Forced colors: the rules were read, not drawn in a forced-colors browser ([T1d](../ready/T1d-live-app-forced-colors-reduced-motion-and-cpu-gpu-tab.md)). In forced colors the primary and plain buttons look alike, as before.
- `prefers-color-scheme`: the stylesheet has no media query for it. The bootstrap script and `applyTheme` in `core.js` copy the setting into `data-theme`, which I set directly. I did not toggle the OS setting.
- Real startup items, history rows and sudo captures: all faked.
- The favicon is still the old violet. L2 (PR 88) replaces it; the no-violet test skips the `rel="icon"` line until then, and that skip must go when 88 lands.
- The canvas boards were not re-read; the deviations compare with the values in the brief.
- Pre-existing and untouched: `web/page-overview.js` uses an undefined token, `var(--mono,monospace)`.
