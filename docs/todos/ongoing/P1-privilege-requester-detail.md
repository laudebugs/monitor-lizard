# P1: Show which process and script requested sudo

| Field | Value |
|---|---|
| ID | `P1` |
| Status | `ongoing` |
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
- N3 gave every table export, a column chooser and Copy row; those must carry the new detail.

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

- [ ] Tests first (Rust and Node), then the code.
- [ ] Enrich hops; extend `PrivEvent`; wire the server.
- [ ] Draw the detail; extend `EXPORT` and `TABLE_META`.
- [ ] Look at it in Chrome in both themes with synthetic rows in a scratch HOME.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A row written before this change (hop with three fields) | Loads; the detail says the field "was not recorded", never blank |
| A row with no chain | "(parent not captured)" |
| `/usr/bin/env node a.js`, `bash -c "..."`, `python3 -m pkg`, `node --require x a.js` | Label names the script or flag, not the inline code |
| Command containing `<`, `&`, quotes | Shown as text, exported intact, never in the URL hash |
| Working directory unreadable | "not readable (another user's process or it had exited)" |
| Authd row, pid reused long after | No match outside the window |
| Authd row, no live row, process gone | "launched before Monitor Lizard was running or exited before capture" |

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
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** hops now store start time, terminal and working directory (`detail` marks rows written by this build); `watcher::hop_label` and the page's `hopLabel` add the script name for interpreters, checked against one shared fixture; `requesterApp` names the app from the chain; the Privilege page has an App column, an expandable detail under each launch and each matched failed authorization, and export-only "Full chain" columns (CSV text, JSON array, Copy row). `privilege::match_live` and `resolve` join authd rows to live rows; an unmatched one says it was launched before Monitor Lizard was running or exited before capture.
- **Compatibility:** all new `Hop` fields default, so no migration; a database test loads an old-format row, an empty chain and a row with `<`, `&` and quotes.
- **Tests:** 9 new Rust tests in `watcher`, 9 in `privilege`, 1 in `history`; 14 Node tests in `tests/web/requester.test.mjs` plus one in `export.test.mjs`. `make check`, `make test` and `make check-docs` pass.
- **Browser (Chrome, scratch HOME, synthetic rows, light and dark):** detail rows, hostile text shown as text, toggle, row-copy buttons still one per row, export columns and the secrets note. Failed-authorization rows were injected into page memory, because the real authd log cannot be seeded.
- **Not verified:** a real `sudo` capture end to end (the task forbade running it), so cwd, tty and start time for a live chain are covered by tests on this test process's own ancestors only; a narrow window; keyboard walk-through; forced colors.
- **Open:** a command is cut at 300 characters; an interpreter label takes the first non-option word.
