# ownwatch

Local-first macOS monitor that groups every process and network socket by app and flags anything that is not yours.

    pnpm install      # only for typecheck
    node src/cli.ts   # grouped snapshot + findings
    node src/server.ts  # http://127.0.0.1:4747 (dashboard in progress)

Needs Node >= 22.18. See docs/PLAN.md.
