# Resolution: the Storage page scrolls sideways at narrow widths

Detail: [detail.md](detail.md)

## The fix

Each of the two lists sits in one scroll box (`.sbox`, `overflow-x: auto`). The box's inner element has a minimum width worked out from the list's own tracks: the sum of the track minimums, plus 12 px per gap, plus 32 px of row padding. That is 568 px for the clean-ups and 396 px for System Data. The numbers are computed in `web/page-storage.js` (`rowFloor`) from the same track lists that sit beside each grid definition, so the floor changes when a track does.

The box is a keyboard stop with `role="region"` and the name "<list>, scrollable", the way `web/table.js` names its scrolling tables, and it gets the same focus outline as `.tbl`. The clean-ups header now has the same 16 px side padding as its rows, so header cells and columns line up.

## How it was checked

Measured in same-origin iframes of each width, dark theme, after a real scan of this Mac (411 items). The first table is the page width, `document.documentElement.scrollWidth`.

| Check | Before | After |
|---|---|---|
| Page width at 400 px | 597 | 400 |
| Page width at 600 px | not measured | 600 |
| Page width at 795 px | not measured | 795 |
| Page width at 1000 px | not measured | 1000 |
| Page width at 1352 px | not measured | 1352 |

The second table is each scroll box's visible width and content width, which show where it scrolls. In every row the Review button ended inside its row, and the first and fourth column headers sat at the same x as their cells.

| Width | Clean-ups box | System Data box |
|---|---|---|
| 400 | 310 / 568 | 310 / 396 |
| 600 | 510 / 568 | 510 / 510 |
| 795 | 485 / 568 | 485 / 485 |
| 1000 | 690 / 690 | 690 / 690 |
| 1352 | 641 / 641 | 641 / 641 |

There is no regression test: the page has no browser test harness, so this stays a recorded manual check.

## Left for later

- The widths were measured in iframes. A screenshot of the narrow layout was not taken, because the browser kept the window wide.
- Folder-name truncation at 400 px, and a keyboard scroll of each box, were not tried by hand.
- The box is a tab stop at every width, including when nothing scrolls. `table.js` adds the stop only when its table overflows; doing the same here needs a resize observer.
- The S2a and S2b todos still list their own narrow-window and review gaps.
