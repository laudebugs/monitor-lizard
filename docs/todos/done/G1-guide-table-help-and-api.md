# G1: Guide table, `help` command and `/api/guide`

| Field | Value |
|---|---|
| ID | `G1` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D8 |
| Depends on | None |
| ADRs | None |
| Branch | `feat/g1-guide-table-help-and-api` |
| PR title | `feat(guide): one command table behind help and /api/guide` |
| Size | M (200-400) |
| PR | pending |

## Goal

Put every command and term the user may need in one Rust table, and use it for `sys-monitor help` and `GET /api/guide`, replacing the hand-written usage line in `src/main.rs`.

## Context

- `src/main.rs` lists subcommands in a single usage string that is easy to forget to update.
- The design canvas board "E · Command guide" has the fields: id, topic, title, command, plain description, example output, safety (`read-only`, `changes files`, `needs sudo`), where it appears in the dashboard, how to undo, search tags.

## Scope

**In scope:**

- `src/guide.rs` with `COMMANDS` and `TERMS` as static tables and a `Safety` enum.
- Entries for every `sys-monitor` subcommand.
- Entries for the macOS and developer commands on the design board: `df -h /`, `diskutil apfs list`, `tmutil listlocalsnapshots /`, `tmutil deletelocalsnapshots <date>`, `du -sh ~/Library/* | sort -h`, `xcrun simctl delete unavailable`, `docker system df`, `docker system prune`, `brew cleanup -n`.
- `sys-monitor help [word]` prints the table, filtered by the word.
- `GET /api/guide` returns both tables as JSON.
- A test that every `main.rs` subcommand has a table entry.

**Out of scope** (name the todo that covers it):

- The page: G2. Running any command: never.

## Target files

| File | Change |
|---|---|
| `src/guide.rs` | (new) tables, search helper |
| `src/main.rs` | `help` subcommand; usage generated from the table |
| `src/server.rs` | `/api/guide` |

## Steps

- [x] On a real Mac, run each read-only command and copy real output into the example (trim, mark with a date). Read the manual page of each command that changes data and check the flags and the confirmation text.
- [x] Write the table-completeness test first.
- [x] Generate the usage line from the table.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| `sys-monitor help snapshot` | Entries whose title, command or tags contain the word |
| `help` with no match | "No command matches" and the topic list |
| Unknown subcommand | Usage generated from the table, exit code unchanged |
| A command in the table needs a path or date | Placeholder is shown in angle brackets, never a real path from this Mac |

## Acceptance criteria

- Adding a subcommand without a table entry fails `make test`.
- No example output contains a real user name, host or path from the author's Mac.

## Tests

Rust tests, 6 minimum.

## Verification

```sh
make check
make test
cargo run --release -- help snapshot
```

## Docs to update

`docs/agents/commands.md`, README command list, plan status.

## Risks and open questions

- Command output formats change between macOS versions; each example carries the macOS version it was captured on.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

**2026-11-26.** Built `src/guide.rs` with `COMMANDS` (12 `sys-monitor` entries, 9 macOS and developer entries), `TERMS` (9), a `Safety` enum and a search helper. `sys-monitor help [word]` prints matches, or "No command matches" and the topic list. An unknown subcommand prints a usage line generated from the table, with the exit code unchanged (0). `GET /api/guide` returns both tables as JSON through one route line in `src/server.rs`.

Ten Rust tests cover completeness in both directions, unsupported arm shapes, unique ids, search, no-match text, usage, placeholders and absence of home paths. Docs changed: `docs/agents/commands.md`, README, plan.

**Deviations and open items:**

- `docker system df` has no example, because the Docker daemon was not running. Capture one when Docker is up.
- `xcrun simctl delete unavailable`, `docker system prune` and `tmutil deletelocalsnapshots` delete data, so they were not run. Their entries come from `simctl help`, `docker system prune --help` and `man tmutil`.
- The man page does not say `deletelocalsnapshots` needs root. The label `needs sudo` is the safe assumption; confirm it against a real snapshot.
- `tmutil listlocalsnapshots /` listed no snapshots on the capture Mac, so its example is the header line.
- Examples were captured on macOS 27.0.1. The `diskutil apfs list` example is trimmed and shows `<uuid>`; the `du` example shortens the home folder to `~`.
- The `sys-monitor` entries have no examples yet.
- The completeness test reads the dispatch `match` in `src/main.rs` and rejects any arm that is not `Some("name")`.
