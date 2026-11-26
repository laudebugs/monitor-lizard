# network-table-columns-collapse-at-narrow-width

At 400 px the first three Network table columns shrank to about one character, and the Apps and Listeners tables clipped columns at 795 px. Apps also squeezed at 1352 px with its inspector open. Fixed: every flexible column has a minimum and the table scrolls inside its card.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, measurements, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
