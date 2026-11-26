# Resolution: table columns collapse at narrow widths

Detail: [detail.md](detail.md)

## The fix

**What was wrong.** At 400 px the Network table's App, Remote and Ports columns were 6, 7 and 5 px wide. At 795 px they were 0 px. Apps, Listeners and Persistence squeezed the same way.

**Why.** The flexible tracks were `minmax(0, fr)`, so they could shrink to nothing. The only guard was a 640 px row floor, applied at 700 px or less. The fixed tracks it had to clear had grown to 542 px.

**What changed.** Each flexible track now has its own minimum: 80 px, and 120 px for the first (name) column of Apps, Network and Persistence. A row's minimum width comes from its own tracks, the gaps and the padding, in one CSS rule. The script sets `--tracks-min` and `--cols` on the table, after hiding columns, so a table with fewer columns gets a smaller floor. The 640 px rule is gone, so the floor applies at every width.

A table whose floor is wider than its card scrolls inside the card. From 1101 px up, tables are clipped to keep their sticky header, so the script adds the class `xscroll` to a table that does not fit. That table scrolls and its header is not sticky.

**How it was checked.** See the table below. Rows, cells and Copy buttons still line up on Apps, Network, Listeners, Persistence and History, and hiding a column still lowers the floor.

**What is left.** See the end of this page.

## How it was checked

Cell widths are the header cells in px, measured in same-origin frames, dark theme. "Row" is the row's width; where it is larger than the card, the table scrolls inside the card. The document scroll width equalled the viewport width in every case, before and after.

| Page and width | Card | Before: cells | After: row | After: cells |
|---|---|---|---|---|
| Network 400 | 342 | 6, 7, 5, 80, 90, 110, 190 | 902 | 120, 80, 80, 80, 90, 110, 190 |
| Network 795 | 517 | 0, 0, 0, 80, 90, 110, 190 | 902 | 120, 80, 80, 80, 90, 110, 190 |
| Network 1000 | 722 | 32, 38, 29, 80, 90, 110, 190 | 902 | 120, 80, 80, 80, 90, 110, 190 |
| Network 1352 | 1073 | 146, 173, 133, 80, 90, 110, 190 | 1073 | unchanged |
| Persistence 400 | 342 | 0, 110, 70, 0, 100, 100, 190 | 922 | 120, 110, 70, 80, 100, 100, 190 |
| Persistence 795 | 517 | 0, 110, 70, 0, 100, 100, 190 | 922 | 120, 110, 70, 80, 100, 100, 190 |
| Persistence 1352 | 1073 | 158, 110, 70, 194, 100, 100, 190 | 1073 | unchanged |
| Listeners 795 | 517 | 80, 60, 15, 22, 80, 120 | 640 | 80, 60, 80, 80, 80, 120 |
| Listeners 1000 | 722 | 80, 60, 101, 141, 80, 120 | 722 | unchanged |
| Apps 400 | 342 | 70, 56, 56, 61, 53, 96, 96 | 736 | 120, 56, 56, 80, 80, 96, 96 |
| Apps 795 | 517 | 23, 56, 56, 20, 18, 96, 96 | 736 | 120, 56, 56, 80, 80, 96, 96 |
| Apps 1000 | 722 | 101, 56, 56, 89, 76, 96, 96 | 736 | 120, 56, 56, 80, 80, 96, 96 |
| Apps 1352, inspector open | 651 | 74, 56, 56, 65, 56, 96, 96 | 736 | 120, 56, 56, 80, 80, 96, 96 |

Also measured at 600 px for every page, with and without the Apps inspector at 600, 795 and 1000 px: the same widths as at 400 px.

**Looked at in a browser** (screenshots, iframe of the stated width, the view scrolled to the table): Network at 400 px dark, and at 795 px light; Listeners at 795 px dark; Persistence at 400 px light; Apps at 400 px dark; Apps at 1352 px with the inspector open, dark and light. Columns were readable and the table scrolled inside its card. Network, Persistence and Listeners at 1352 px, and the other theme at each width, were measured but not looked at.

**Alignment.** A script compared DOM rows, cells, grid tracks and Copy buttons on Apps, Network, Listeners, Persistence and History at 400 and 1000 px. Counts matched, and each button was within 1 px of its row's centre. Hiding the first column through the column menu kept every table aligned. On Network the floor fell from 862 to 761 px. That run used the 80 px name columns; a second run after the 120 px change repeated the counts for Apps, Network and Persistence.

**Regression test.** `tests/web/table-floor.test.mjs` reads the grids and the floor block from `web/index.html`. It fails if any flexible track has a minimum under 80 px, or if the floor rule or the variables are removed. It cannot show that a table scrolls; the measurements above are the proof of that.

## Left for later

- At 1101 to about 1250 px, Network and Persistence (floors 902 and 922 px) scroll inside their card and lose their sticky header. The Expected column is 190 px; a shorter "Mark" button below a breakpoint would remove the scroll. This needs a change to the button text in script, so it was left out.
- A name longer than about 15 characters is still cut with an ellipsis in a 120 px column. The full name is in the row's Copy output and the inspector.
- Apps at 1352 px with the inspector open now scrolls inside a 651 px card instead of squeezing.
