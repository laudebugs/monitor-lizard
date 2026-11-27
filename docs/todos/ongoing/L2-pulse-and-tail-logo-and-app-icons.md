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
| Size | M (200-400) |
| PR | — |

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
- [x] Look at the favicon and the sidebar mark in a real browser, in both themes.
- [ ] Look at the Dock icon and the menu bar (light and dark) on a real screen: pending.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Favicon at 16 px | Both strokes and the tail's turn stay visible |
| Tray image at 18 and 22 pt, 1x and 2x | One colour, so macOS recolours it; the zigzag and the tail both read |
| Light browser tab | The favicon switches to the light Moss colours through `prefers-color-scheme` |
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
- The system plan lists no row for the L todos, so there is no plan row to update.

## Definition of done

- [x] Claimed: first commit added this file in `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on the prose
- [x] `make check` and `make test` green
- [ ] Dock and menu bar looked at on a real screen
- [ ] Moved `ongoing/` -> `done/` in the last work commit

## Implementation Record

2026-11-27.

**Geometry change.** The mark failed at 16 px in one place. The tail was `M34 38 H56 V26 H46 V30`. Its last segment ended 4 units below the stroke above it, which is 1 px at 16 px, so the curl closed into a solid blob. The tail is now `M34 38 H56 V26 H46`: the hook segment is dropped and nothing else moved. The pulse (`M4 38 H14 L20 18 L28 54 L34 38`), the stroke width of 6, the square caps and the miter joins are as drawn. One geometry is used at every size, so the sidebar, the tray and the Dock icon match the favicon. Thickening the strokes was not tried: it would have closed the gap further.

**Framing.** The mitered tips make the drawing 58 units wide and about 58.4 high, from y 6.1 to 64.5. The bottom tip leaves the 64 by 64 canvas by half a unit. `mark.svg`, `tray.svg` and the sidebar logo therefore use `viewBox="0 5 60 60"`, which holds the whole mark.

**Files.**

- `icon.svg` keeps the L1 construction: a 824 px rounded square at (100, 100) with a corner radius of 185, a vertical gradient from #12231c to #0b1410, and a faint inner outline. The mark is scaled 9.5 times, so it is about 551 px wide (67% of the ground), and its bounding box is centred on the canvas.
- `mark.svg` draws the pulse in #4fd18b and the tail in #b8f2d2, and switches to #17803f and #0f5f2c under `prefers-color-scheme: light`.
- `tray.svg` draws both strokes in black.
- `scripts/make-icons.sh` no longer calls `cargo tauri icon`, whose `.icns` differed on every run. It renders the iconset sizes with `inkscape` and builds the `.icns` with `iconutil`. Two runs in a row gave identical files (SHA-1 compared for all six images and `web/index.html`). Its outputs are the same set of files as before, at the same sizes.
- The sidebar logo uses `currentColor` for the pulse and `var(--logo-accent, var(--acc))` for the tail.

**Before and after.** Before, a 12 by 12 grid of violet and mint cells formed a lizard on a violet squircle; at 16 px it was a stub with two notches, and the sidebar showed a branching path. After, a green zigzag with a pale tail sits on a near-black green squircle. At 16 px the zigzag and the squared tail are both visible. The tray image is a single black line. The sidebar mark is one violet line today, because `--logo-accent` is not set yet.

**What I looked at.** I looked at renders of the app icon at 1024, 512, 128, 32 and 16 px; the favicon at 16, 32 and 128 px on a dark and a light ground; and the tray image at 18, 22, 36 and 44 px in black on a light ground and in white on a dark one (the macOS template rule draws it white on a dark bar). In a real browser, served from a scratch HOME, I looked at the sidebar logo in the dark and light themes and confirmed that `--logo-accent` recolours the tail. I also drew the favicon data URI as an image on both grounds. `make check`, `make test` (492 Rust tests and the Node tests) and `make check-docs` passed.

**Known flaws.**

- Where the tail starts, its square cap sticks out 3 units behind the join and shows as a small pale notch beside the pulse's last stroke. Painting the pulse on top moves the flaw to a green speck above the tail, so the drawing as specified stays. The owner may want to move the start of the tail.
- At 16 px the app icon's tail is faint against the ground. The favicon is the better small-size drawing.

**Not verified.** The Dock icon, the menu bar in light and dark, and the favicon in a real tab: the browser extension swaps in its own tab icon, so the tab strip could not be seen. The favicon under a dark OS setting was checked only as a render with the dark colours, because the browser here was in light mode. No `cargo tauri build` was run, so the `.icns` was not seen inside a built `.app`.
