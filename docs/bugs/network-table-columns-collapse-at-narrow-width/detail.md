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

From the CSS, by arithmetic; the numbers were not re-measured in a browser this round.

- The Network row grid has four fixed tracks (80, 90, 110 and 190 px), three flexible tracks and six 12 px gaps. The fixed tracks and gaps come to 542 px.
- A table with the copy button gets 16 px left and 64 px right padding on each row, so 80 px.
- At a viewport of 700 px or less the row has a 640 px floor. That leaves 640 - 80 - 542 = 18 px for the three flexible tracks, about 6 px each. In the 400 px screenshots the first three columns were about one character wide.
- Above 700 px the floor does not apply. At 795 px the Apps and Listeners tables have no floor and clip their columns.

## Also affected

At 1352 px with the inspector open on Apps, the table is squeezed. App names are cut ("Google Ch...", "Claude Co...", "identityse...") and the Listening and Signing columns clip. The owner's session saw this in dark and light.

Apps has no Expected column, so the cause there may differ. A hypothesis: the open inspector takes about 380 to 460 px of the width, which leaves the table about the same width as a narrow window. This was not measured.

## Cause

A hypothesis, not confirmed by a fix. Two changes pulled against each other. One PR added the 64 px right padding and the 640 px row floor to stop this collapse. A later PR added a 190 px Expected column to Network and Persistence.

The fixed tracks grew from about 280 px to about 470 px. The 640 px floor is the same, so the flexible columns went from about 270 px to about 18 px in total.

The rule `.tbl.has-copy>.tr` (specificity 0,3,0) beats `.tr.ell` (0,2,0), so the floor wins whenever the copy button is present. The `ell` class is not the cause.

## Likely fix direction

Raise the floor in proportion to the fixed tracks, so it never leaves the flexible columns under a readable width. Or let the Expected column shrink, or move it into the inspector, at narrow widths. The 700 px limit on the floor also needs a look for Apps and Listeners. This is left for its own pull request.
