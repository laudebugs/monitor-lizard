# What people ask of a system monitor, and where Mac disk space goes

As of 2026-11-26. Gathered the same day by reading Hacker News threads through the Algolia API and by web search. This note did not go through the five-step pipeline in [../README.md](../README.md): there is no fact-check log, and the items under "Not verified" are unchecked.

**Thesis:** people distrust single-number metrics, cannot see what fills their disk, and want a verdict they can act on and data they can take out of the app. The threads below support the Storage page, the Guide and the navigation rules in the [plan](../../plans/storage-and-guide.md).

## Read directly (Hacker News comments)

| Thread | What the comments say |
|---|---|
| [Stats, a macOS menu bar monitor](https://news.ycombinator.com/item?id=42881342) | Users compare it with a paid alternative and say they only use a few widgets. One reports a confusing first run because most menu bar items were hidden for lack of space. One prefers the paid app for sensor coverage. |
| [The %CPU number in Activity Monitor](https://news.ycombinator.com/item?id=42094889) | A long argument over what % CPU should mean when clock speed changes. One reply prefers Linux's pressure stall information to percentages because it shows where the bottleneck is. |
| [What "Energy Impact" measures](https://news.ycombinator.com/item?id=10120155) | The author concludes the figure is inaccurate and in arbitrary units. Others say it still pushed developers to cut wakeups. |
| [Leaves, a disk usage treemap](https://news.ycombinator.com/item?id=48936389) | Commenters ask for quick actions and a shareable HTML or SVG report. They say `du` on a home folder is too slow, and that such a tool should already be installed when the disk fills. |
| [Don't trust Activity Monitor on M1](https://news.ycombinator.com/item?id=31233157) | Only the title was seen. |

## From search summaries, pages not opened (secondary)

- Apple Community and Eclectic Light describe "System Data" as everything not listed elsewhere, only approximately. Users report 100 to 500 GB in it, local snapshots counted inside it, and a Storage pane stuck on "Calculating". [Eclectic Light](https://eclecticlight.co/2025/02/11/what-is-system-data-in-storage-settings/), [an Apple Community thread](https://discussions-kr-prz.apple.com/thread/255914364).
- Product Hunt comments on [MacStats](https://www.producthunt.com/posts/macstats) say monitors dump every number until you tune them out, and praise grouping helper processes under their app. A comment on [FindDiskKiller](https://www.producthunt.com/posts/finddiskkiller) asks for per-app disk writes over 24 hours or a week.

## What each complaint changes

| Complaint | Design response | Todo |
|---|---|---|
| Headline numbers mislead | Every number names its source; "unavailable" with a reason, never 0 | S1, T1b |
| System Data is a mystery | Break it into named, sized items with a safety label | S1, S2b |
| Too many numbers, no verdict | A one-line verdict as each page's title | T1b, R1 to R3b |
| Hard to find things across pages | URL state, one search box | N1, N2 |
| Cannot take the data out | CSV and JSON export, copy a row as JSON | N3 |
| Want per-app history | Opt-in hourly disk writes | S5 |
| Unclear which commands to run | A searchable guide that never runs commands | G1, G2 |

## Not verified

Whether per-app disk writes and lifetime SSD writes are readable without root on macOS 27. The numbers quoted from secondary sources. Whether the HN sample represents most users: it is a few threads, mostly from people who install monitors.
