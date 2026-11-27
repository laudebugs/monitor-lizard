# The Storage page scrolls sideways at narrow widths

Resolution: [resolution.md](resolution.md)

## What we saw

In a 454 px window (the narrowest this Chrome allows) the whole Storage page scrolled sideways. In "Suggested clean-ups" the Size values and the Review buttons ran past the card's right edge.

## How to reproduce

1. Run `monitor-lizard serve` and open `http://127.0.0.1:<port>/#storage` after a scan finishes.
2. Narrow the window, or load the page in a 400 px wide iframe on the same origin.
3. Compare `document.documentElement.scrollWidth` with the window width.

## Measurements

In a 400 px frame: page width 597 px, and the last Review button ended at 597 px. The clean-ups list was 310 px wide and its rows 310 px, but their five tracks need 536 px. After the clean-ups list was boxed, the page was still 425 px: the "Inside System Data" rows, whose three tracks need 364 px, ended at 425 px.

## Cause

Both lists are grids whose tracks have minimum widths (`minmax(120px, …)`, `84px`, and so on) that add up to more than a narrow card. Neither sat in a scroll container, so the grid widened the page. The Network, Apps and Persistence tables already scroll inside their card for this reason.
