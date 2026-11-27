# The Storage page scrolls sideways at narrow widths

Resolution: [resolution.md](resolution.md)

## What we saw

In a 454 px window, the narrowest this Chrome allows, the whole Storage page scrolled sideways. In "Suggested clean-ups" the Size values and the Review buttons ran past the card's right edge.

## How to reproduce

1. Run `monitor-lizard serve` and open `http://127.0.0.1:<port>/#storage` after a scan finishes.
2. Narrow the window, or load the page in a 400 px wide iframe on the same origin.
3. Compare `document.documentElement.scrollWidth` with the window width.

## Measurements

In a 400 px frame the page was 597 px wide and the last Review button ended at 597 px. The clean-ups list was 310 px wide, but each row needs 568 px: five tracks with minimums of 120, 120, 74, 90 and 84 px, four 12 px gaps, and 16 px of padding on each side.

After the clean-ups list was boxed with a 540 px minimum, the page was still 425 px wide. The "Inside System Data" rows needed 396 px (tracks 110, 80 and 150, two gaps, 32 px of padding). A review then showed that 540 was also too small by the 28 px that the padding adds: the box's content measured 552 px.

## Cause

Both lists are grids whose tracks have minimum widths that add up to more than a narrow card, and neither sat in a scroll container, so the grid widened the page. The Network, Apps and Persistence tables already scroll inside their card for the same reason.

A second, older fault showed up in the same list: the column header had no side padding while each row has 16 px, so every header cell sat 16 px left of its column.
