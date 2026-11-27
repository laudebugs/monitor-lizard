# L1: Scale-grid app icon, favicon and tray icon

| Field | Value |
|---|---|
| ID | `L1` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0004](../../decisions/0004-tauri-menu-bar-shell.md) |
| Branch | `feat/l1-scale-grid-app-icon` |
| PR title | `feat(shell): add the scale-grid lizard as app icon, favicon and tray icon` |
| Size | M (200-400) |
| PR | — |

## Goal

Replace the flat blue placeholder icon with a mark that fits the name: a pixel lizard built from square cells, like the Storage treemap. Use it for the app icon, the dashboard favicon and the menu-bar tray.

## Context

- The owner chose concept E (scale grid) from five options on 2026-11-27. The options were drawn as SVG and compared at 168, 64, 32 and 16 px.
- The shell (ADR 0004) bundled one 512 px blue square and showed text only in the menu bar.

## Scope

**In scope:**

- Source SVGs in `src-tauri/icons/source/` and `scripts/make-icons.sh` to rebuild the PNGs and `.icns`.
- `tauri.conf.json` lists the macOS icon files; the tray shows `tray.png` as a template image next to its text.
- The favicon in `web/index.html` is the same mark as a data URI.

**Out of scope** (name the todo that covers it):

- A wordmark or README banner: not planned.
- Windows, iOS and Android icons: the app is macOS only.

## Steps

- [x] Draw the three SVGs from one 12 by 12 cell map.
- [x] Generate the icon set and remove the files macOS does not use.
- [x] Wire the tray icon (`image-png` feature) and the bundle icon list.
- [x] Replace the favicon.
- [x] Build the app bundle and look at it.

## Acceptance criteria

- The built `.app` carries `icon.icns`.
- The tray icon is black and clear, so macOS can recolour it.
- `make check` is green.

## Verification

```sh
make check
cd src-tauri && cargo tauri build --debug --bundles app
```

## Definition of done

- [x] Claimed in the first commit
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`
- [x] `make check` green

## Implementation Record

2026-11-27.

The mark is 12 by 12 cells. The app icon draws each cell 46 of 52 units wide with a gap and alternates two tints for a scale texture; the eyes are the two mint cells in the top row. The favicon and tray icon draw the cells touching, because a gap of a fraction of a pixel blurs at 16 px. The tray icon has no tints and its eyes are holes. At 18 pt the head reads as a narrow stub with two notches, not as eyes.

The icon body is 824 px on a 1024 px canvas, which is Apple's grid, so the Dock does not show it larger than other icons.

Not checked: how the icon looks in the Dock and in a light and a dark menu bar on a real screen. The bundle was built and the icons were inspected as files.
