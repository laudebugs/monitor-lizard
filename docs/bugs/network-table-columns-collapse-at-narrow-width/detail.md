# Network table columns shrink to one character at a narrow width

Resolution: none yet. This bug is unresolved.

## What we saw

At 400 px, the first three columns of the Network table (App, Remote, Ports) are about one character wide, so each row reads "C [. 4". At 795 px, the Apps table clips its Listening and Signing headers, and the Listeners table clips Bind address and App. Found on 2026-11-26 during the browser sweep of the design tokens todo.

## How to reproduce

1. Build `main` at `4a56e50` and serve it with `monitor-lizard serve 4961`.
2. In a Chrome tab, open `http://127.0.0.1:4961/#network`.
3. Load the page in a 400 px wide same-origin frame, or look at it in a window 795 px wide for Apps and Listeners.

Dark and light themes look the same. The document itself does not scroll sideways (scroll width 400); the table scrolls inside its card.

## Measurements

- The Network row grid has four fixed tracks (80, 90, 110 and 190 px) and three flexible tracks. Those four tracks, six 12 px gaps and the 32 px row padding come to 574 px.
- The table card is about 344 px wide at 400 px, so the flexible tracks get nothing and collapse to their `minmax(0, ...)` floor.
- The table has no minimum width of its own. Rows use the class `tr ell`, and `.tr.ell` sets `min-width:0`.

## Cause

Probable, not yet confirmed by a fix. The `ell` row class lets rows shrink below the sum of their fixed tracks, so the flexible columns lose all their space. The rule that keeps a 640 px minimum on narrow screens (`.tbl.has-copy>.tr`) is overridden by `.tr.ell`. Two definitions of the row minimum width disagree. The fix belongs with the table-style work in the page-shell todo, because it touches every table.
