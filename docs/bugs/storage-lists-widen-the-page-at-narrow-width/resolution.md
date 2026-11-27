# Resolution: the Storage page scrolls sideways at narrow widths

Detail: [detail.md](detail.md)

## The fix

Each of the two lists sits in a box with `overflow-x: auto`, and the list inside it has a minimum width equal to what its tracks need: 540 px for the clean-ups, 370 px for System Data. A card narrower than that scrolls inside itself, the way the other tables do.

## How it was checked

Measured in same-origin iframes of each width, dark theme, after a real scan of this Mac (343 items). The page width is `document.documentElement.scrollWidth`. "Box" is the scroll box's visible width and its content width.

| Width | Page before | Page after | Clean-ups box after | System Data box after |
|---|---|---|---|---|
| 400 | 597 | 400 | 310 / 552 | 310 / 380 |
| 600 | not measured | 600 | 510 / 552 | 510 / 510 |
| 795 | not measured | 795 | 485 / 552 | 485 / 485 |
| 1000 | not measured | 1000 | 690 / 690 | 690 / 690 |
| 1352 | not measured | 1352 | 641 / 641 | 641 / 641 |

At 1000 and 1352 px both lists fit, so nothing scrolls there. There is no automated test: the page has no browser test harness, so this stays a recorded manual check.

## Left for later

The check was in iframes, not in a window of that width, and a screenshot of the narrow layout was not taken because the browser kept the window wide. The folder names in the clean-ups list were not checked for truncation at 400 px.
