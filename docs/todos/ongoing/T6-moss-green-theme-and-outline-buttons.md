# T6: Moss green theme and outline buttons

| Field | Value |
|---|---|
| ID | `T6` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 and D12 |
| Depends on | T1a, T1b |
| ADRs | None |
| Branch | `feat/moss-theme-outline-buttons` |
| PR title | `feat(web): switch to the Moss green theme with outline buttons` |
| Size | M (200-400) |
| PR | — |

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
| `docs/plans/storage-and-guide.md` | Decision D12 and the T6 row |
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

`docs/plans/storage-and-guide.md` (D12, T6 row), `docs/agents/layout.md`.

## Risks and open questions

- Forced colors are checked by reading the rules only (T1d does the live check).
- Whether the owner wants the danger button's hover to differ from the plain one is not on the boards; it keeps a solid fill with a `--fg` border.

## Definition of done

- [x] Claimed: first commit moved this file into `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
