# Monitor Lizard

Local-first macOS monitor that groups every process and network socket by app and flags anything that is not yours.

    cargo run --release              # grouped snapshot + findings
    cargo run --release -- serve          # dashboard at http://127.0.0.1:4747 (Stop/Quit need the per-run token the page carries)
    cargo run --release -- approve "/Applications/Notion.app"
    cargo run --release -- json      # full snapshot as JSON
    cargo run --release -- privilege 60   # sudo/admin authorization requests, last 60 min
    cargo run --release -- watch          # live: catch every sudo/su launch with its parent chain
    cargo run --release -- sudo-log 20    # stored sudo/su launches
    cargo test

State (baseline and history) lives in `~/.monitor-lizard/`; a one-time migration copies whichever of `~/.sys-monitor/` or `~/.ownwatch/` is found, the first time it finds no new folder. The history database carries a schema version; the first time a build versions an older file it keeps a copy as `history.db.bak`, and a file from a newer build is refused rather than changed. Turning on the `metrics_history` setting adds downsampled CPU/memory history to that same `history.db` (1 minute resolution for 48 hours, 15 minutes for 30 days; never per-process) -- off by default. Measured by filling both tiers to their full retention (23,048 rows: 2880 one-minute buckets plus 2880 fifteen-minute buckets, 4 metric columns each) and checking the resulting file size directly: about 753 KB after `VACUUM` (812 KB before it). The plan is [docs/plans/system-monitor.md](docs/plans/system-monitor.md).

## Commands

`monitor-lizard help [word]` lists every command and term, filtered by the word, with what each one changes and how to undo it. The dashboard serves the same table at `/api/guide`.

## Working on this repo

Rules for people and AI agents are in [AGENTS.md](AGENTS.md). Work comes from the todo board (`make todos`), one todo per branch and pull request, each in its own git worktree. Run `make check` before pushing; CI runs the same targets and one `gate` check. Docs live in [docs/](docs/README.md). Structure adopted from the owner's `scaffold` template.

## Status and safety

macOS only, early. Read-only by default and local-only (binds 127.0.0.1). The dashboard can stop your own processes after a confirmation; it never signals root processes, launchd or itself, and every write needs a per-run token. No telemetry, no network calls. The dashboard's type is bundled (IBM Plex Sans and Mono), so it looks the same whether or not those fonts are installed; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## License

MIT, see [LICENSE](LICENSE).
