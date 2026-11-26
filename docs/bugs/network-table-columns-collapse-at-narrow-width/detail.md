# Network table columns shrink to one character at a narrow width

Resolution: [resolution.md](resolution.md)

## What we saw

At 400 px, the first three columns of the Network table (App, Remote, Ports) are about one character wide, so each row reads "C [. 4". At 795 px, the Apps table clips its Listening and Signing headers, and the Listeners table clips Bind address and App. Found on 2026-11-26 during the browser sweep of the design tokens todo.

## How to reproduce

1. Build `main` at `6fe7b01` and serve it with `monitor-lizard serve 4961`.
2. In a Chrome tab, open `http://127.0.0.1:4961/#network`.
3. Load the page in a 400 px wide same-origin frame, or look at it in a window 795 px wide for Apps and Listeners.

Dark and light themes look the same. The document itself does not scroll sideways (scroll width 400); the table scrolls inside its card.

## Measurements

Measured on 2026-11-26 in Chrome, on `main` at `6fe7b01`. Each width was a same-origin frame of that width. Cell widths are `getBoundingClientRect` of the header cells, in px. The document never scrolled sideways at any width.

| Page and width | Card | Row | Cell widths, left to right |
|---|---|---|---|
| Network 400 | 342 | 640 | **6, 7, 5**, 80, 90, 110, 190 |
| Network 795 | 517 | 517 | **0, 0, 0**, 80, 90, 110, 190 |
| Network 1000 | 722 | 722 | 32, 38, 29, 80, 90, 110, 190 |
| Persistence 400 to 795 | 342 to 517 | 640 or 517 | **0**, 110, 70, **0**, 100, 100, 190 |
| Listeners 795 | 517 | 517 | 80, 60, **15, 22**, 80, 120 |
| Apps 795 | 517 | 517 | **23**, 56, 56, **20, 18**, 96, 96 |
| Apps 1352, inspector open | 651 | 651 | 74, 56, 56, 65, 56, 96, 96 |

The 640 px floor applied only at a viewport of 700 px or less, and only to tables with the Copy button.

## Cause

The hypothesis held for Network and Persistence at 400 px, and the measurements corrected the rest.

**Network at 400 px.** Fixed tracks and gaps come to 542 px and the row padding to 80 px. The 640 px floor leaves 18 px for three flexible tracks, and the measured widths are 6, 7 and 5 px: 18 px in all.

**Wider than 700 px.** There is no floor, so the flexible tracks take whatever is left, down to 0 px. At 795 px the card is only 517 px wide because the sidebar still sits beside it.

**Apps.** Its fixed tracks are small (304 px), so its columns collapse later. The tracks used `minmax(0, fr)`, which allows 0 px.

**Apps with the inspector at 1352 px.** The card is 651 px. The inspector is not the cause. It takes the width, and the table has no floor to hold against it.

The common cause is two sources of truth that disagree. The grid strings decide how wide the columns need to be. A separate magic 640 px, applied at one width range, decides how wide the row may be. The flexible tracks had no minimum of their own.

The hypothesis also said the floor beat `.tr.ell` by specificity. That is true of the old rules and is not the cause.
