# ownwatch

Local-first macOS monitor that groups every process and network socket by app and flags anything that is not yours.

    cargo run --release              # grouped snapshot + findings
    cargo run --release -- serve     # dashboard at http://127.0.0.1:4747
    cargo run --release -- approve "/Applications/Notion.app"
    cargo run --release -- json      # full snapshot as JSON
    cargo test

Baseline lives in `~/.ownwatch/baseline.json`. See docs/PLAN.md.
