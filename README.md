# Monitor Lizard

Local-first macOS monitor that groups every process and network socket by app and flags anything that is not yours.

    cargo run --release              # grouped snapshot + findings
    cargo run --release -- serve          # dashboard at http://127.0.0.1:4747 (Stop/Quit need the per-run token the page carries)
    cargo run --release -- approve "/Applications/Notion.app"   # also: host:<app key>|<address>, startup:<plist path>, finding:<app key>:<rule>#<ports>
    cargo run --release -- json      # full snapshot as JSON
    cargo run --release -- privilege 60   # sudo/admin authorization requests, last 60 min
    cargo run --release -- watch          # live: catch every sudo/su launch with its parent chain
    cargo run --release -- sudo-log 20    # stored sudo/su launches
    cargo run --release -- model install --model qwen3-4b --yes   # downloads and verifies the local classifier model
    cargo test

State (baseline and history) lives in `~/.monitor-lizard/`; a one-time migration copies whichever of `~/.sys-monitor/` or `~/.ownwatch/` is found, the first time it finds no new folder.

The history database carries a schema version. The first time a build versions an older file it keeps an owner-only copy as `history.db.bak`, and a file from a newer build is refused rather than changed.

Turning on the `metrics_history` setting adds downsampled CPU/memory history to that same `history.db` (1 minute resolution for 48 hours, 15 minutes for 30 days; never per-process) -- off by default. Measured by filling both tiers to their full retention (23,048 rows: 2880 one-minute buckets plus 2880 fifteen-minute buckets, 4 metric columns each) and checking the resulting file size directly: about 753 KB after `VACUUM` (812 KB before it). The plan is [docs/plans/system-monitor.md](docs/plans/system-monitor.md).

Marking an item as expected (the dashboard's "Mark expected" button, or `approve`) records it in `baseline.json` and in the history database, and stops it counting as new. A marked host or startup item keeps its row with an "expected" chip, and "Unmark" brings it back. The History page lists every mark and unmark.

A Tauri menu-bar shell (`src-tauri/`) runs the same server and dashboard in a window with a tray item, instead of a terminal and a browser tab: `cd src-tauri && cargo tauri build --bundles app`. See [ADR 0004](docs/decisions/0004-tauri-menu-bar-shell.md).

## Commands

`monitor-lizard help [word]` lists every command and term, filtered by the word, with what each one changes and how to undo it. The dashboard serves the same table at `/api/guide`.

## Tables

Every table in the dashboard can export what is on screen as CSV or JSON (all filtered and sorted rows, shown columns only), choose its columns, and copy one row as JSON. Exports are built in the page and saved by your browser; nothing is sent anywhere. A file lists app names, hosts and paths, so share it with care.

The dashboard's Guide page shows the same table with search, topic filters and a glossary. It copies a command and never runs one.

## Working on this repo

Rules for people and AI agents are in [AGENTS.md](AGENTS.md). Work comes from the todo board (`make todos`), one todo per branch and pull request, each in its own git worktree. Run `make check` before pushing; CI runs the same targets and one `gate` check. Docs live in [docs/](docs/README.md). Structure adopted from the owner's `scaffold` template.

## Status and safety

macOS only, early. Read-only by default and local-only (binds 127.0.0.1). The dashboard can stop your own processes after a confirmation; it never signals root processes, launchd or itself, and every write needs a per-run token. The Storage route can move scanned items to your Trash, never delete them: it takes ids from the last scan, re-checks each item on disk, asks twice for anything labelled yours, and logs every move in History before it happens. Items go to the Trash, but Finder's "Put Back" may not be offered for them; History keeps each original path so you can move one back by hand. No telemetry, and no network calls except `monitor-lizard model install`, which only ever contacts Hugging Face for the one pinned file it was told to fetch. The dashboard's type is bundled (IBM Plex Sans and Mono), so it looks the same whether or not those fonts are installed; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## License

MIT, see [LICENSE](LICENSE).
