# storage-and-guide: where did my disk go, and which commands do I need?

- **Status:** Draft
- **Date:** 2026-11-26
- **Baseline:** `main` at `db79262`, inspected 2026-11-26
- **Asked for by:** laudebugs
- **Builds on:** [system-monitor](system-monitor.md), [resource-monitoring](resource-monitoring.md), [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)
- **Todos:** [S1](../todos/ready/S1-storage-scanner-and-api.md), [S2](../todos/ready/S2-storage-page.md), [S3](../todos/ready/S3-move-to-trash-action.md), [S4](../todos/ready/S4-capacity-history-and-forecast.md), [S5](../todos/ready/S5-per-app-disk-writes.md), [G1](../todos/ready/G1-guide-table-help-and-api.md), [G2](../todos/ready/G2-guide-page.md). The desktop shell is [A17](../todos/ready/A17-tauri-menu-bar-shell.md) and [ADR 0004](../decisions/0004-tauri-menu-bar-shell.md). Navigation and data: [N1](../todos/ready/N1-url-state-and-breadcrumbs.md), [N2](../todos/ready/N2-global-search.md), [N3](../todos/ready/N3-table-export-columns-copy-row.md). Look and layout: [T1](../todos/ready/T1-design-tokens-and-app-shell.md), [R1](../todos/ready/R1-overview-redesign.md), [R2](../todos/ready/R2-network-redesign.md), [R3](../todos/ready/R3-persistence-redesign.md). Baseline: [A9](../todos/ready/A9-mark-as-expected.md).

## 1. Goal

Add two pages to the dashboard that answer questions the app cannot answer today. **Storage** shows where disk space went, how fast it is growing, and which items are safe to remove. **Guide** is a searchable list of the commands and terms a user runs into, with what each one changes. Success: a user with a nearly full disk can find the three biggest safe items and move them to the Trash in under a minute, and nothing is removed without a preview and an audit entry.

## 2. What the system does today

As of 2026-11-26, on `main`:

- `src/sampler.rs` keeps CPU, memory and network samples in a ring buffer and serves them at `/api/metrics` (B1 is done). Disk metrics are todo B2; the Disk and network tab is B7.
- `src/server.rs` has one guarded write, `POST /api/kill`, that needs a matching Origin and the per-run token, and records an action in `History`.
- `src/persistence.rs` is the pattern for a scanner whose result is cached and served as JSON.
- `src/main.rs` has a hand-written usage line; there is no `help` command.
- `web/index.html` is one file with a `NAV` list, no build step and bundled IBM Plex fonts.

## 3. What others do

From the 2026-11-26 research (see the design canvas "Sys Monitor Redesign Directions", board "Research and directions"): people report macOS "System Data" as an unexplained 100 to 500 GB, a Storage pane stuck on "Calculating", and monitors that show many numbers and no verdict. On HN, commenters want a disk tool that is always installed, quick actions, shareable reports and per-app history. Quotes from Product Hunt and Apple Community came from search summaries and are secondary. Apple's own documentation was not read.

## 4. Decisions

- **D1.** Storage is its own nav item. The Performance page's Disk tab (B7) shows rates and capacity; Storage shows what fills the disk. Both use one capacity bar component.
- **D2.** The scanner reads a fixed list of known locations plus a bounded search for `node_modules`. It never walks the whole disk. Anything it cannot read is shown as unavailable with a reason, never as 0 (resource-monitoring D1).
- **D3.** Sizes are allocated bytes (`st_blocks * 512`), not apparent size, so a sparse file such as `Docker.raw` is not over-counted.
- **D4.** Labels come from a rules table, like `rules::classify`: `safe` only for locations on an allowlist of things an app rebuilds or re-downloads; `review` where removal has side effects; `yours` for user files. A model never sets the label (system-monitor D4).
- **D5.** Removal moves items to the Trash. The browser sends an item id from the latest scan, never a path. The server re-resolves it, refuses symlinks and anything outside the allowlist or the home folder, and refuses an item that changed since the scan. Every move is recorded in `History`.
- **D6.** Growth history is capacity samples in SQLite through B9: system-wide, one value a day, kept 90 days. The forecast is a straight-line fit over the last 14 days and says "collecting history" before 7 days exist.
- **D7.** Per-app disk writes are opt-in and off by default. They store hourly byte deltas per app group for 7 days, can be erased from Settings, and never store file names. This keeps the resource-monitoring rule that per-process history is not written by default.
- **D8.** The Guide's commands live in one Rust table that feeds `sys-monitor help`, `GET /api/guide` and the page, so they cannot drift. The page copies commands and never runs one.
- **D9.** Look and feel follow design direction D (violet console, treemap, suggested clean-ups), the Guide board, and the Overview, Network and Persistence boards, all in the owner's canvas "Sys Monitor Redesign Directions" (2026-11-26). Fonts stay the bundled IBM Plex. The owner decided on 2026-11-26 that the whole app takes this look, so T1 restyles every page and R1 to R3 redesign three of them; the other four pages (Apps, Listeners, Privilege, History) have no design yet.
- **D10.** Two options stay undecided and have their own spike: how to move to the Trash without a new permission prompt (S3), and whether the Tauri shell is worth its signing cost (ADR 0004). The owner approved the Tauri spike (A17a) on 2026-11-26 and accepted its recommendation the same day: ADR 0004 is amended and stays Proposed, and acceptance depends on the owner's Full Disk Access test (A17).
- **D11.** Every page has the same anatomy, so data is easy to find and read: breadcrumb and search, a one-line verdict as the title, filters with counts, a table, and an inspector with Related links and the rule that flagged an item.
- **D12.** Page state lives in the URL hash (page, filters, sort, selected item), so a view can be bookmarked, shared and restored on reload (N1). Counts and "Related" links open pre-filtered views.
- **D13.** One search box, focused with `/`, finds apps, ports, hosts, startup items and Guide entries across pages (N2).
- **D14.** Every table can export what is on screen as CSV or JSON, choose its columns and copy a row as JSON; times show relative text with the absolute time on hover; status is always text, never color alone (N3).
- **D15.** Owner decision 2026-11-26: the opt-in per-app disk writes setting in D7 is accepted.

## 5. Order of work

| Todo | Title | Depends on |
|---|---|---|
| S1 | Storage scanner and `/api/storage` | B2 |
| S2 | Storage page | S1, B7, T1 |
| S3 | Move to Trash action | S2 |
| S4 | Capacity history and forecast | S1, B9 |
| S5 | Per-app disk writes (opt-in) | S2, B3, B9 |
| G1 | Guide table, `help` and `/api/guide` | None |
| G2 | Guide page | G1 |
| A17 | Tauri menu-bar shell | ADR 0004 accepted |
| N1 | URL state and breadcrumbs | None |
| N2 | Global search | N1, G1 |
| N3 | Table export, column chooser, copy row | None |
| A9 | Mark as expected (feedback baseline) | None |
| T1 | Design tokens and app shell for every page | None |
| R1 | Overview redesign | T1, N1, A9 |
| R2 | Network redesign | T1, N1, N3, A9 |
| R3 | Persistence redesign | T1, N1, N3, A9 |

G1, G2, N1, N3, A9 and T1 can start now. The S chain waits for B2, B7 and B9. R4 (Apps, Listeners, Privilege and History) is not written because those pages have no design yet.

## 6. Risks and open questions

1. **Snapshot sizes.** The design shows 21 GB for local snapshots. `tmutil listlocalsnapshots /` lists dates only. Whether a size is readable without root is unverified; if not, the row shows the count and says why there is no size.
2. **Purgeable space.** The "important usage" figure that separates purgeable from free is marked unverified in the resource-monitoring research. S1 starts by checking it.
3. **Full Disk Access.** Mail and Messages folders need it. Without it those rows are unavailable. A signed app bundle (A17) would give one stable identity to grant it to, which is unverified.
4. **Scan cost.** A slow scan would make the monitor its own top consumer. S1 sets a time budget and runs at low priority.
5. **Design gap.** Apps, Listeners, Privilege and History have no redesign. Until they do, T1 gives them the new tokens and shell but not the new layout.
6. **A9 is new scope.** The designs have "Mark as expected" buttons for hosts, startup items and alerts. The feedback baseline is listed as not yet written in the system-monitor plan; A9 is its first slice.
