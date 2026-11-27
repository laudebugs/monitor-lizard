# P1: Show which process and script requested sudo

| Field | Value |
|---|---|
| ID | `P1` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md), section 2 (`src/privilege.rs`, `src/watcher.rs`) and D3 (read-only) |
| Depends on | None |
| ADRs | None |
| Branch | `feat/p1-privilege-requester-detail` |
| PR title | `feat(privilege): show which process and script requested sudo` |
| Size | L (over 400), kept as one PR because the parts share one data shape |
| PR | — |

## Goal

The owner saw a failed `sudo shutdown -h 0` row whose requester chain read `bash < node < node < zsh < claude < zsh < login` and asked "can I see what process/script triggered it?". Show, for each sudo/su launch and each failed authorization, every process between the person and `sudo`: pid, full command line, executable, working directory, start time and terminal, plus a one-line "app" summary.

## Context

- `src/watcher.rs` already stores per hop pid, executable path and command (at most 300 characters), but the page printed only the first word of each command.
- `src/privilege.rs` reads the authd log. A failed authorization found its parent with `ps` only while the process lived, otherwise the page said "(exited)".
- The authd line names the client pid, which is the pid of `sudo` itself, so a live-capture row for the same pid and time explains it.
- N3 added export, a column chooser and Copy row to every table; those must carry the new detail.

## Scope

**In scope:**

- Each hop also records start time, controlling terminal and working directory where the kernel lets a normal user read them. Old rows still load.
- A label that adds the script name when the first word is an interpreter (`node script.js`, not `node`).
- An "app" summary derived from the chain (Terminal, iTerm, an editor, Claude).
- An expandable detail under each live row and each failed authorization that has a chain.
- A pure function that matches an authd row to a live-capture row by pid and time window.
- Export (CSV, JSON, Copy row) carries the full chain. The "may contain secrets" warning stays.

**Out of scope:**

- Endpoint Security, which would catch launches that start and exit between two polls.
- Reading another user's process details (needs root).
- Running or probing `sudo` in any way.

## Target files

| File | Change |
|---|---|
| `src/watcher.rs` | `Hop` gains optional `started`, `tty`, `cwd`, `detail`; enrichment parsing; `hop_label` |
| `src/privilege.rs` | `PrivEvent` gains `epoch`, `source`, `chain`; `resolve` and `match_live` as pure functions |
| `src/history.rs` | Old-row compatibility test for `recent_sudo` |
| `src/server.rs`, `src/main.rs` | Pass the live rows to `privilege::recent`; CLI uses `hop_label` |
| `tests/fixtures/hop-labels.json` | (new) label cases shared by the Rust and Node tests |
| `web/index.html` | `requester` block, Privilege page detail rows, export columns |
| `tests/web/requester.test.mjs` | (new) Node tests for the helpers |
| `docs/agents/layout.md`, `docs/plans/system-monitor.md` | Updated |

## Steps

- [x] Write the tests with the code (Rust and Node).
- [x] Enrich hops; extend `PrivEvent`; wire the server.
- [x] Draw the detail; extend `EXPORT` and `TABLE_META`.
- [x] Look at it in Chrome in both themes with synthetic rows in a scratch HOME.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A row captured before P1 (hop with three fields) | Loads; the detail says the field "was not recorded", never blank |
| A row with no chain | "(parent not captured)" |
| `/usr/bin/env node a.js`, `bash -c "..."`, `python3 -m pkg`, `node --require x a.js` | Label names the script or flag, not the inline code |
| Command containing `<`, `&`, quotes | Shown as text, exported intact, never in the URL hash |
| Working directory unreadable | "not readable (the process had exited, or belongs to another user)" |
| Authd row, pid reused long after | No match outside the window |
| Authd row, no live row, process gone | "no matching launch recorded (launched before Monitor Lizard was running, exited before capture, or older than the records kept)" |

## Acceptance criteria

- A failed `sudo` row on the Privilege page shows its chain with full commands when a live row matches.
- The detail lists pid, command, executable, working directory, start time and terminal for every hop, and an app line.
- CSV, JSON and Copy row hold the full chain; the secrets warning is still shown.
- No command line appears in the URL.

## Tests

Rust: parsing of `ps` rows into hop detail, `hop_label` over the shared fixture, the time-window match, old-row JSON, a database round trip. Node: label, app summary, hop facts, export text and JSON, URL state ignoring the expanded state.

## Verification

```sh
make check
make test
make check-docs
```

## Docs to update

`docs/agents/layout.md`, `docs/plans/system-monitor.md`.

## Risks and open questions

- A command is cut at 300 characters, so a long argument list loses its tail; the page says when it was cut.
- The interpreter label is a heuristic: it takes the first argument that is not an option, and knows a short list of options that take a value.
- Terminal presence is a hint of a person at the keyboard, not proof: a script can allocate a terminal.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md` (two reviews of PR #59); findings and fixes in the second commit's message
- [x] `/writing-review` run on any prose (round 2 of PR #59; its findings are fixed in the commit after 3eb56ee)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built**

- Each hop stores start time, terminal, working directory and a cut marker; `detail` marks rows captured since P1. Rows captured before P1 load unchanged, with no migration (a database test covers an old-format row, an empty chain and a row with `<`, `&` and quotes).
- `watcher::hop_label` and the page's `hopLabel` add the script name for interpreters; one shared fixture checks both. `requesterApp` names the app from the chain, as a guess.
- The Privilege page has an App column, an expandable detail under each launch and each failed authorization with a chain or a live parent, and export-only "Full chain" columns (CSV text, JSON array, Copy row).
- `privilege::match_live` and `resolve` join authd rows to live rows. A row with no match says "no matching launch recorded (launched before Monitor Lizard was running, exited before capture, or older than the records kept)".
- The page string "not recorded (captured by an older version)" can only fire for a row with `detail: false`, and every row captured since P1 has `detail: true`, so it cannot appear for a current row.

**Tests:** Rust in `watcher`, `privilege` and `history`; Node in `tests/web/requester.test.mjs` and `export.test.mjs`. `make check`, `make test` and `make check-docs` pass.

**Review round 1 (PR #59), fixed**

- Safety: one `visible` in Rust and JS turns bidi, zero-width and control characters into visible escapes in the page, the exports and the CLI, with `<bdi>` around values.
- Accessibility: the toggle has its own name, `aria-controls` only while the detail exists, and focus survives a poll redraw.
- Limits: a cut command is flagged in the table and the exports (code points on both sides); a 12-hop chain says it was cut; the `ps` parent line is cut at 300 characters, with one `ps` call per refresh.
- Output: the JSON export of an unmatched authorization carries its statement and the app, the same as CSV and Copy.
- Labels and app: more interpreters and value options in the label; app names match whole names and read "probably".
- Code: `cwd_of` safety, `parse_log_time` without byte slicing, `reqOpen` pruned, collision-free element ids.

**Review round 2 (writing review):** wording of the unmatched, cut-chain, terminal and unreadable texts made consistent; docs tightened.

**Changes beyond the review's findings**

- The coordinator's round-1 note asked for these to be recorded; the author made each:
- Removed the inline `min-width:640px` on the two Privilege tables, so the table floor sets the width (done in the first commit so the table scrolls like the others; not requested).
- Added the App column, which changed the grid (done in the first commit to answer "which app").
- The `monitor-lizard privilege` command now opens the history database to read live rows (needed for the same join the page does).
- The join reads at most 500 live rows (`LIVE_JOIN_LIMIT`, chosen to bound the read), so an authorization older than the 500th most recent sudo launch shows as unmatched.

**Browser:** Chrome, scratch HOME, synthetic rows, light and dark, window not resized. Seen: detail rows, a 12-hop chain with the cut note, a 320-character command flagged as cut, bidi and zero-width text as escapes, focus kept after a redraw, `aria-controls` absent while closed, one Copy button per row, export columns and the secrets note. Failed-authorization rows were injected into page memory, because the real authd log cannot be seeded.

**Not verified:** a real sudo capture end to end (cwd, tty and start time for a live chain are tested only on the test process's own ancestors); a screen reader reading the toggle's name; forced colors; a narrow browser window; the keyboard walk-through; CLI output with hostile text (covered by the shared `visible` test only).

**Open:** the interpreter label takes the first word that is not an option, with a short list of options that take a value.
