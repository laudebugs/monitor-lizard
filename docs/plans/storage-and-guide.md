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
- **D5.** Removal moves items to the Trash. The browser sends an item id from the latest scan, never a path. The server re-resolves it and refuses a symlink, anything not inside the home folder or `/Applications`, an item not owned by the current user, and an item that changed since the scan. Every real move is recorded in `History` before it happens (D15).
- **D6.** Growth history is one capacity sample a day, system-wide, kept 90 days in its own SQLite table. It is written even when the B9 metrics-history setting is off, because it holds nothing about apps. The forecast is a straight-line fit over the last 14 days and says "collecting history" before 7 days exist. A one-day fall in used space of more than 5% of the disk counts as a clean-up, and the fit then uses only the days after it (a judgement call, recorded 2026-11-26).
- **D7.** Per-app disk writes are opt-in and off by default. They store hourly byte deltas per app group for 7 days, can be erased from the setting's control on the Storage page, and never store file names. This keeps the resource-monitoring rule that per-process history is not written by default. The owner accepted this setting on 2026-11-26.
- **D8.** The Guide's commands live in one Rust table that feeds `monitor-lizard help`, `GET /api/guide` and the page, so they cannot drift. The page copies commands and never runs one.
- **D9.** Look and feel follow design direction D (violet console, recoloured green by D16, treemap, suggested clean-ups), the Guide board, and the Overview, Network and Persistence boards, all in the owner's canvas "Sys Monitor Redesign Directions" (2026-11-26). Fonts stay the bundled IBM Plex. The owner decided on 2026-11-26 that the whole app takes this look, so T1a and T1b restyle every page and R1, R2 and R3b redesign three of them; the other four pages (Apps, Listeners, Privilege, History) have no design yet.
- **D10.** S3a chose `NSFileManager` `trashItemAtURL` through the `trash` crate (2026-11-27): it raises no permission prompt, but it may not show "Put Back" in Finder. The Finder route keeps "Put Back" and asks for an Automation permission, so it was not used. The Trash route may not offer "Put Back" in Finder, so every reply says so and History keeps the original path. ADR 0004 is Accepted (the owner's Full Disk Access test passed, 2026-11-26) and A17 (the shell) is built.
- **D15.** The removal contract (follow-up to S3a; S3b builds on it):
  - **Preview first.** `POST /api/trash?ids=` with no `dry`, or `dry=1`, is a preview. Only `dry=0` moves; any other `dry` value, or a repeated one, is a 400. A preview that would move something returns `confirm_token` (valid 10 minutes, single use, in memory, never logged or stored). A real move sends `dry=0&confirm_token=...` for exactly the same ids; a token is bound to the ids, their labels, the scan and `confirm_yours`, and carries the ids the preview showed as movable: a different selection, a changed label, a newer scan or a different `confirm_yours` is 409 `confirmation_mismatch`, and an item that was not movable in the preview is refused (`not_previewed`) even if it would pass now. At most 32 previews wait at once; more get 429 `too_many_previews`. This is S3b's rule that nothing moves before a preview of the same selection has been shown. Items labelled `yours` also need `confirm_yours=1` on both calls (the preview shows them as refused without it).
  - **No audit row, no move.** Before anything is checked, one `trash_items` row per id is written as `pending` in one transaction; just before each move it becomes `moving`; afterwards `moved`, `refused`, `failed` or `not_attempted`. If a write before a move fails, that item and the rest are not attempted and the reply is a 500. At the next start any row left `pending` or `moving` becomes `interrupted` (it may or may not have moved). The table keeps about the newest 10,000 rows, trimmed by whole request. A dry run writes nothing.
  - **Checks twice.** The first pass also re-measures (size, entry count, no mount inside). Only the cheap path checks (no symlink, canonical form and case, inside home or `/Applications`, owned by the current user, not a mount point, mtime) run again right before the mover; the re-measure does not. A folder with another volume mounted inside is refused. `HOME` must be the account's home folder (canonicalised), owned by the current user, at least two levels below the root.
  - **Bounded.** The route runs on its own thread, one at a time (409 `busy` otherwise), with a 45 s budget for the whole request; items not reached are `not_attempted` with code `time_budget`.
  - **Reply (additive).** `ok` (true only when every item moved or would move and the audit was written), `dry_run`, `history_id`, `counts`, `items` (per item: `status` of `would_move`, `moved`, `refused`, `failed` or `not_attempted`, a machine-readable `code`, `reason`, `bytes`), `failed`, `moved`, `refused`, `confirm_token`, `audit_error`, `note`. All items refused is still a 200 with `ok: false`. A pre-check refusal does not stop the batch; a failed mover or a failed audit write does. Errors are `{ok:false, code, error}`.
  - `GET /api/history` returns `trash_moves` as requests with their items and shows paths under home as `~/...`; the database keeps full paths.
- **D11.** Every page has the same anatomy: breadcrumb and search, a one-line verdict as the title, filters with counts, a table, and an inspector. The inspector has Related links and the rule that flagged the item. The shared shell in T1b provides it.
- **D12.** Page state lives in the URL hash (page, filters, sort, selected item), so a view can be bookmarked, shared and restored on reload (N1). Counts and "Related" links open pre-filtered views.
- **D13.** One search box, focused with `/`, finds apps, ports, hosts, startup items and Guide entries across pages (N2).
- **D14.** Every table can export what is on screen as CSV or JSON, choose its columns and copy a row as JSON (N3). Times show relative text with the absolute time on hover, and status is always text, never color alone.
- **D16.** The app theme is green (2026-11-27): the owner chose the Moss palette with option B (outline) buttons on boards J, K, L and M of the same canvas. Green is the brand and the selection colour (`--acc`), so the OK status moved to cyan (`--ok`): a healthy chip must never look like a selected one. Buttons are square with a 2px border and a transparent fill. T6 delivers it.
- **D17.** The storage scan converges across runs (2026-11-27). Each location keeps its last size, whether it was complete, when it was measured and its top-level modification time; a partial scan never lowers a complete earlier size. The server's background scan runs for 120 s at 20% of one core; the command-line `storage` stays at 20 s. The owner said "let's do S1b", which approves the work. The 120 s figure is the todo's proposal and the owner has not chosen it separately: it is one constant (`SERVER_TIME_BUDGET` in `src/storage/cache.rs`) to change.

## 5. Order of work

| Todo | Title | Depends on |
|---|---|---|
| S1 | Storage scanner and `/api/storage` | B2 |
| S1b | Converging storage scan | S1 |
| S2a | Storage page: status strip, treemap, table, inspector | S1, B7, T1a |
| S2b | Storage page: suggested clean-ups, Inside System Data | S2a |
| S3a | Move to Trash route and audit log | S1 |
| S3b | Move to Trash on the page (built; the owner's real-move check on a throwaway folder checked out on 2026-11-27, without further detail; stays ongoing while S2a is) | S2a, S3a |
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
| T1c | Check forced colors and reduced motion (done 2026-11-27, emulated on Linux) | T1a |
| T1d | Live-app forced colors, reduced motion and the CPU and GPU tab (the owner's DevTools check) | T1c |
| T2 | Split the dashboard page into CSS and script files (after S4) | S4 |
| T3 | Resizable (expandable) table columns (done 2026-11-27) | T1b, N3 |
| T4 | A mobile-width layout direction (done 2026-11-27) | T1b |
| T6 | Moss green theme and outline buttons (D16; done 2026-11-27) | T1a, T1b |
| T5 | Split `history.rs` into modules | None |
| T1b | Page shell, sidebar groups, table styles (done 2026-11-26; forced colors in T1c) | T1a, N1 |
| R1 | Overview redesign | T1b, N1, A9 |
| R2 | Network redesign | T1b, N1, N3, A9 |
| R3a | Persistence data: schedule, fingerprints, removals | M1 |
| R3b | Persistence page redesign | R3a, T1b, N1, N3, A9 |

S1b is built (2026-11-27, PR open). S4 is built (2026-11-26, PR pending merge). S1 is built (2026-11-26, PR pending merge). N1 is delivered and G1 is built (2026-11-26, PR pending merge). G2 is built (2026-11-26, PR pending merge). N2 is built (2026-11-27, PR open; browser check partly done). N3 and A9 are built and merged, and so is M1. R3a is built and merged (2026-11-26). T1a is done after a browser sweep (2026-11-26). A17 is built (2026-11-26): the owner's Full Disk Access test passed, ADR 0004 is Accepted, and the shell runs the server and dashboard in a window with a tray. The S chain waits for B2, B7 and B9. R4 (Apps, Listeners, Privilege and History) is not written because those pages have no design yet.

## 6. Risks and open questions

1. **Snapshot sizes.** The design shows 21 GB for local snapshots. `tmutil listlocalsnapshots /` lists dates only. Whether a size is readable without root is unverified; if not, the row shows the count and says why there is no size.
2. **Purgeable space.** The "important usage" figure that separates purgeable from free is marked unverified in the resource-monitoring research. S1 starts by checking it.
3. **Full Disk Access.** Mail and Messages folders need it. Without it those rows are unavailable. The signed app bundle (A17) gives one stable identity to grant it to, confirmed by the owner's Full Disk Access test (ADR 0004).
4. **Scan cost.** A slow scan would make the monitor its own top consumer. S1 sets a time budget and runs at low priority.
5. **Design gap.** Apps, Listeners, Privilege and History have no redesign. Until they do, T1a and T1b give them the new tokens and shell but not the new layout.
6. **A9 is new scope.** The designs have "Mark as expected" buttons for hosts, startup items and alerts. The feedback baseline is listed as not yet written in the system-monitor plan; A9 is its first slice, built 2026-11-26: typed baseline entries, an audit table, and buttons on the Overview, Network and Persistence pages.
7. **Migrations exist (M1, built 2026-11-26).** `History` versions its schema with `PRAGMA user_version`, so a column can be added to an existing table by appending a step to `MIGRATIONS` (R3a does). New tables, as in S3a, S4 and S5, need no step of their own.
8. **T4 was design-then-build, unlike the design gap in item 5.** T4 measured the real narrow-width behavior first (the collapsed-sidebar problem it actually found was not the one originally guessed) and recorded that design pass in its own Implementation Record before the CSS, rather than waiting on the owner's canvas the way R4 does.
