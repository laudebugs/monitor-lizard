# L2: Pulse-and-tail logo and app icons

| Field | Value |
|---|---|
| ID | `L2` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0004](../../decisions/0004-tauri-menu-bar-shell.md) |
| Branch | `feat/logo-pulse-tail` |
| PR title | `feat(brand): replace the logo and app icons with the pulse-and-tail mark` |
| Size | S plus generated binaries (about 200 changed text lines; six regenerated images) |
| PR | [#88](https://github.com/laudebugs/monitor-lizard/pull/88) |

## Goal

Replace the pixel lizard with the owner's chosen logo, concept D ("Pulse and tail"): a heartbeat line that ends in a curled tail. Use it for the app icon, the favicon, the menu-bar tray image and the sidebar logo, in the Moss palette.

## Context

- [L1](../done/L1-scale-grid-app-icon.md) built the icon pipeline (`scripts/make-icons.sh`, three SVG sources, the tray template image). This todo changes the drawings and keeps the pipeline.
- The owner chose concept D on 2026-11-27 and chose the Moss palette. The mark was drawn on a design canvas and had not been checked at small sizes.
- [ADR 0004](../../decisions/0004-tauri-menu-bar-shell.md) holds the shell, including the tray template image rule.
- The app is moving to the green Moss theme in a separate todo, so the sidebar logo must work with the current violet `--acc` and with a later green one.

## Scope

**In scope:**

- `mark.svg`, `icon.svg` and `tray.svg` redrawn from the new geometry, and every file derived from them.
- `scripts/make-icons.sh` made independent of the pixel mark and reproducible.
- The sidebar logo in `web/index.html`.

**Out of scope** (name the todo that covers it):

- The green Moss theme of the app: a separate todo; this one only adds `var(--logo-accent, var(--acc))` so the tail can follow it.
- A wordmark or README banner: not planned.

## Target files

| File | Change |
|---|---|
| `src-tauri/icons/source/mark.svg`, `icon.svg`, `tray.svg` | redrawn from the pulse-and-tail geometry |
| `src-tauri/icons/icon.icns`, `icon.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `tray.png` | regenerated |
| `scripts/make-icons.sh` | builds with `inkscape` and `iconutil` only; byte-identical on every run |
| `web/index.html` | favicon data URI and the sidebar `.logo` SVG |
| `docs/agents/layout.md` | the icon row describes the new mark |

## Steps

- [x] Render the mark at 16, 32 and 128 px on the dark and light grounds; fix what fails at 16 px.
- [x] Redraw the three sources; regenerate through `scripts/make-icons.sh`.
- [x] Replace the sidebar logo.
- [x] Look at the sidebar mark in a real browser, in both themes.
- [ ] Look at the favicon in a real browser tab, in a light and a dark tab strip: pending (only drawn as an image so far).
- [ ] Look at the Dock icon and the menu bar (light and dark) on a real screen: pending.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Favicon at 16 px | The zigzag reads; the coil closes into a block (accepted by the owner) |
| Tray image at 18 and 22 pt, 1x and 2x | One colour, so macOS recolours it; the zigzag reads and the tail is a small block |
| Light OS scheme | The favicon switches to the light Moss colours through `prefers-color-scheme` |
| Forced colors | The sidebar mark draws in `CanvasText` (rule in `web/app.css`) |
| Violet theme today | The tail follows `--acc`, so the sidebar logo is one colour until the Moss theme sets `--logo-accent` |

## Acceptance criteria

- `scripts/make-icons.sh` run twice gives identical files.
- The tray image is black on transparent, with no other colour.
- No remote asset: the favicon stays a data URI.
- `make check`, `make test` and `make check-docs` are green.

## Tests

No unit tests: the change is drawings and wiring. The files are checked by looking at renders and in a browser.

## Verification

```sh
scripts/make-icons.sh && git status --short   # twice; the second run changes nothing
make check
make test
make check-docs
```

## Docs to update

`docs/agents/layout.md`.

## Risks and open questions

- Dock and menu bar rendering on a real screen has not been seen.
- The coil closes at 16 px; the owner accepted that.
- The favicon follows the OS colour scheme, not the browser tab theme. A light OS with a dark browser theme shows dark green on a dark tab (contrast 2.1 to 3.3 against #1f1f1f).
- The system plan lists no row for the L todos, so there is no plan row to update.

## Definition of done

- [x] Claimed: first commit added this file in `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages (PR #88 reviews, second commit)
- [x] `/writing-review` run on the prose
- [x] `make check` and `make test` green
- [ ] Dock and menu bar looked at on a real screen
- [ ] Moved `ongoing/` -> `done/` in the last work commit

## Implementation Record

2026-11-27.

**Geometry.** The owner asked for a continuous mark, with the lighter green bleeding into the darker one. The mark is now one path, `M1 38H14L20 18L28 54L34 38H56V26H46V30`, 6 units wide with butt caps and miter joins, stroked with one `linearGradient` (`gradientUnits="userSpaceOnUse"`, x 1 to 56): the main colour at offset .35 and the accent at .7. The earlier two-stroke construction and its clipPath are gone, and with them the seam, the notch and the ledge. As specified, the pulse started at `M4 38` with a square cap; `M1 38` with a butt cap puts the left edge in the same place.

The tail is the owner's original coil, `H56V26H46V30`, unchanged. The owner chose it knowingly: its last segment ends 4 units above the baseline stroke, so it closes into a blob at 16 px. At 16 px the tail reads as a solid block beside the zigzag; at 32 px a thin slit shows inside it; from 64 px up the coil reads clearly. The geometry was not lengthened or softened to keep it open at small sizes.

The mitered tips make the drawing 58 units wide and 58.4 high, from y 6.1 to 64.5, so the bottom tip leaves the 64 by 64 canvas. `mark.svg`, `tray.svg` and the sidebar logo use `viewBox="0 5 60 60"` for that reason. One geometry is used at every size.

**Tail options considered.** Before the owner's decision, three other tails were drawn as continuous strokes and viewed at 16, 32, 64 and 512 px on both grounds and as an 18 px tray image: an open hook `M34 38H56V26H43` (clean at 16 px, but it reads as a bracket), a taller square curl `M34 38H56V14H41V25` (its hole survives at 16 px), and a round curl `M34 38H48a8 8 0 1 0-8-8` (most tail-like large, soft at 16 px). The owner kept the original coil.

**Files.**

- `icon.svg` keeps the L1 construction: a 824 px rounded square at (100, 100) with a corner radius of 185, a vertical gradient from #12231c to #0b1410, and a faint inner outline. The mark is scaled 9.5 times, about 551 px wide (67% of the ground), centred on its bounding box.
- `mark.svg` is the single gradient path. The stops take #4fd18b and #b8f2d2 from CSS classes, and #17803f and #0f5f2c under `prefers-color-scheme: light`.
- `tray.svg` is the single path in black, with no gradient.
- `scripts/make-icons.sh` no longer calls `cargo tauri icon`, whose `.icns` differed on every run. It renders with `inkscape` and packs with `iconutil`, checks that both exist, and prints Inkscape's output when it fails. Two runs in a row gave identical SHA-1s for all six images and `web/index.html`, with Inkscape 1.4.4 and macOS 27.0.1 (build 26A434) on this machine only.
- The sidebar logo is the same path with a gradient whose stops are `var(--acc)` and `var(--logo-accent, var(--acc))`. A `forced-colors: active` rule in `web/app.css` draws the path in `CanvasText`. The stroke is a presentation attribute (`stroke="url(#logo-g)"`), which any author rule overrides, so the rule needs no `!important`. This was checked by reading the rule against the markup, not in a forced-colors browser.

**Favicon colours.** `prefers-color-scheme` follows the OS, not the browser's tab theme. Contrast against a dark tab (#1f1f1f) and a light one (#f0f0f0): #4fd18b 8.5 and 1.7; #b8f2d2 13.1 and 1.1; #17803f 3.3 and 4.4; #0f5f2c 2.1 and 6.8. A single set can reach about 3.8 on both at best, for a mid green near #1f9a55 (4.6 and 3.2), and the pale accent cannot, so a single set would lose the two-tone look. The media query stays; a light OS with a dark browser theme gives the dark greens on a dark tab, a known limit.

**Before and after.** Before, a 12 by 12 grid of violet and mint cells formed a lizard on a violet squircle; at 16 px it was a stub with two notches, and the sidebar showed a branching path. After, a green zigzag that blends into a pale square coil sits on a near-black green squircle. The tray image is a single black line. The sidebar mark is one violet line today, because `--logo-accent` is not set yet.

**What I looked at.** Renders of the app icon at 1024, 512, 128, 32 and 16 px (bounding box 551 by 555 px, centred); the gradient mark at 16 and 32 px on the dark and the light ground; the blend is smooth and the former notch is gone; and the tray image at 18, 22, 36 and 44 px in black on light and white on dark. In a real browser, from a scratch HOME, the sidebar logo in the dark and light themes, and `--logo-accent` recolouring the tail. The favicon data URI was drawn as an image in the page. `make check`, `make test` and `make check-docs` passed.

**Not verified.** The Dock icon and the menu bar, light and dark. The favicon in a real tab (the browser extension swaps in its own tab icon). The favicon under a dark OS setting (seen only as a render). The forced-colors rule in a forced-colors browser. The `.icns` inside a built `.app` (no `cargo tauri build`).
