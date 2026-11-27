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
- [ ] Look at the Dock icon and the menu bar (light and dark) on a real screen.

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

To be written when the checks are finished.
