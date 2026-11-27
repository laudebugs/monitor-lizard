# storage-and-guide: where did my disk go, and which commands do I need?

- **Status:** Draft
- **Date:** 2026-11-26
- **Baseline:** `main` at `db79262`, inspected 2026-11-26
- **Asked for by:** laudebugs
- **Builds on:** [system-monitor](system-monitor.md), [resource-monitoring](resource-monitoring.md), [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md)
- **Todos:** the table in section 5. The desktop shell is [ADR 0004](../decisions/0004-tauri-menu-bar-shell.md) and [A17](../todos/done/A17-tauri-menu-bar-shell.md).

## 1. Goal

Add two pages to the dashboard that answer questions the app cannot answer today. **Storage** shows where disk space went, how fast it is growing, and which items are safe to remove. **Guide** is a searchable list of the commands and terms a user runs into, with what each one changes. Success: a user with a nearly full disk can find the three biggest safe items and move them to the Trash in under a minute, and nothing is removed without a preview and an audit entry.

## 2. What the system does today

As of 2026-11-26, on `main`:

- `src/sampler.rs` keeps CPU, memory and network samples in a ring buffer and serves them at `/api/metrics` (B1 is done). Disk metrics are todo B2; the Disk and network tab is B7.
- `src/server.rs` has three writes that need a matching Origin and the per-run token: `POST /api/kill`, `/api/approve` and `/api/settings`. Only `/api/kill` records to `History`.
- `src/persistence.rs` is the pattern for a scanner whose result is cached and served as JSON.
- `src/main.rs` has a hand-written usage line; there is no `help` command.
- `web/index.html` is one file with a `NAV` list, no build step and bundled IBM Plex fonts.

## 3. What others do

The evidence is in the [research note](../research/storage-and-navigation-complaints/README.md). People report macOS "System Data" as an unexplained 100 to 500 GB, a Storage pane stuck on "Calculating", and monitors that show many numbers and no verdict. On HN, commenters want a disk tool that is always installed, quick actions, shareable reports and per-app history. Product Hunt and Apple Community points came from search summaries and are secondary. Apple's own documentation was not read.

## 4. Decisions

- **D1.** Storage is its own nav item. The Performance page's Disk tab (B7) shows rates and capacity; Storage shows what fills the disk. Both use one capacity bar component.
- **D2.** The scanner reads a fixed list of known locations plus a bounded search for `node_modules`. It never walks the whole disk. Anything it cannot read is shown as unavailable with a reason, never as 0 (resource-monitoring D1).
- **D3.** Sizes are allocated bytes (`st_blocks * 512`), not apparent size, so a sparse file such as `Docker.raw` is not over-counted.
- **D4.** Labels come from a rules table, like `rules::classify`: `safe` only for locations on an allowlist of things an app rebuilds or re-downloads; `review` where removal has side effects; `yours` for user files. A model never sets the label (system-monitor D4).
- **D5.** Removal moves items to the Trash. The browser sends an item id from the latest scan, never a path. The server re-resolves it, refuses symlinks and anything outside the allowlist or the home folder, and refuses an item that changed since the scan. Every move is recorded in `History`.
- **D6.** Growth history is one capacity sample a day, system-wide, kept 90 days in its own SQLite table. It is written even when the B9 metrics-history setting is off, because it holds nothing about apps. The forecast is a straight-line fit over the last 14 days and says "collecting history" before 7 days exist. A one-day fall in used space of more than 5% of the disk counts as a clean-up, and the fit then uses only the days after it (a judgement call, recorded 2026-11-26).
- **D7.** Per-app disk writes are opt-in and off by default. They store hourly byte deltas per app group for 7 days, can be erased from the setting's control on the Storage page, and never store file names. This keeps the resource-monitoring rule that per-process history is not written by default. The owner accepted this setting on 2026-11-26.
- **D8.** The Guide's commands live in one Rust table that feeds `monitor-lizard help`, `GET /api/guide` and the page, so they cannot drift. The page copies commands and never runs one.
- **D9.** Look and feel follow design direction D (violet console, treemap, suggested clean-ups), the Guide board, and the Overview, Network and Persistence boards, all in the owner's canvas "Sys Monitor Redesign Directions" (2026-11-26). Fonts stay the bundled IBM Plex. The owner decided on 2026-11-26 that the whole app takes this look, so T1a and T1b restyle every page and R1, R2 and R3b redesign three of them; the other four pages (Apps, Listeners, Privilege, History) have no design yet.
- **D10.** One choice is still open: S3a must pick a way to move items to the Trash without a new permission prompt. ADR 0004 is Accepted (the owner's Full Disk Access test passed, 2026-11-26) and A17 (the shell) is built.
- **D11.** Every page has the same anatomy: breadcrumb and search, a one-line verdict as the title, filters with counts, a table, and an inspector. The inspector has Related links and the rule that flagged the item. The shared shell in T1b provides it.
- **D12.** Page state lives in the URL hash (page, filters, sort, selected item), so a view can be bookmarked, shared and restored on reload (N1). Counts and "Related" links open pre-filtered views.
- **D13.** One search box, focused with `/`, finds apps, ports, hosts, startup items and Guide entries across pages (N2).
- **D14.** Every table can export what is on screen as CSV or JSON, choose its columns and copy a row as JSON (N3). Times show relative text with the absolute time on hover, and status is always text, never color alone.

## 5. Order of work

| Todo | Title | Depends on |
|---|---|---|
| S1 | Storage scanner and `/api/storage` | B2 |
| S1b | Converging storage scan | S1 |
| S2a | Storage page: status strip, treemap, table, inspector | S1, B7, T1a |
| S2b | Storage page: suggested clean-ups, Inside System Data | S2a |
| S3a | Move to Trash route and audit log | S1 |
| S3b | Move to Trash on the page | S2a, S3a |
| S4 | Capacity history and forecast | S1 |
| S5 | Per-app disk writes (opt-in) | S2a, B3, B9 |
| G1 | Guide table, `help` and `/api/guide` | None |
| G2 | Guide page | G1 |
| A17 | Tauri menu-bar shell | None (held by ADR 0004) |
| N1 | URL state and breadcrumbs | None |
| N2 | Global search | N1, G1 |
| N3 | Table export, column chooser, copy row | None |
| A9 | Mark as expected (feedback baseline) | None |
| M1 | SQLite schema versioning | None |
| T1a | Design tokens, buttons and chips (done; forced-colors and reduced-motion checks moved to T1c) | None |
| T1c | Check forced colors and reduced motion (the owner's DevTools check) | T1a |
| T2 | Split the dashboard page into CSS and script files (after S4) | S4 |
| T1b | Page shell, sidebar groups, table styles (done 2026-11-26; forced colors in T1c) | T1a, N1 |
| R1 | Overview redesign | T1b, N1, A9 |
| R2 | Network redesign | T1b, N1, N3, A9 |
| R3a | Persistence data: schedule, fingerprints, removals | M1 |
| R3b | Persistence page redesign | R3a, T1b, N1, N3, A9 |

S4 is built (2026-11-26, PR pending merge). S1 is built (2026-11-26, PR pending merge). N1 is delivered and G1 is built (2026-11-26, PR pending merge). G2 is built (2026-11-26, PR pending merge). N3 and A9 are built and merged, and so is M1. R3a is built and merged (2026-11-26). T1a is done after a browser sweep (2026-11-26). A17 is built (2026-11-26): the owner's Full Disk Access test passed, ADR 0004 is Accepted, and the shell runs the server and dashboard in a window with a tray. The S chain waits for B2, B7 and B9. R4 (Apps, Listeners, Privilege and History) is not written because those pages have no design yet.

## 6. Risks and open questions

1. **Snapshot sizes.** The design shows 21 GB for local snapshots. `tmutil listlocalsnapshots /` lists dates only. Whether a size is readable without root is unverified; if not, the row shows the count and says why there is no size.
2. **Purgeable space.** The "important usage" figure that separates purgeable from free is marked unverified in the resource-monitoring research. S1 starts by checking it.
3. **Full Disk Access.** Mail and Messages folders need it. Without it those rows are unavailable. The signed app bundle (A17) gives one stable identity to grant it to, confirmed by the owner's Full Disk Access test (ADR 0004).
4. **Scan cost.** A slow scan would make the monitor its own top consumer. S1 sets a time budget and runs at low priority.
5. **Design gap.** Apps, Listeners, Privilege and History have no redesign. Until they do, T1a and T1b give them the new tokens and shell but not the new layout.
6. **A9 is new scope.** The designs have "Mark as expected" buttons for hosts, startup items and alerts. The feedback baseline is listed as not yet written in the system-monitor plan; A9 is its first slice, built 2026-11-26: typed baseline entries, an audit table, and buttons on the Overview, Network and Persistence pages.
7. **Migrations exist (M1, built 2026-11-26).** `History` versions its schema with `PRAGMA user_version`, so a column can be added to an existing table by appending a step to `MIGRATIONS` (R3a does). New tables, as in S3a, S4 and S5, need no step of their own.
