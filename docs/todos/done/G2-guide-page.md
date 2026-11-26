# G2: Guide page

| Field | Value |
|---|---|
| ID | `G2` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D8, D9 |
| Depends on | G1 |
| ADRs | None |
| Branch | `feat/g2-guide-page` |
| PR title | `feat(web): add the searchable Guide page` |
| Size | M (200-400) |
| PR | [#39](https://github.com/laudebugs/monitor-lizard/pull/39) |

## Goal

Add a Guide nav item with a search box, topic filters, a Commands list with a detail pane, and a Glossary, matching the design board "E · Command guide".

## Context

- G1 serves `GET /api/guide`.
- The dashboard's existing search box (`S.q`, `FILTERABLE`) can feed the Guide, so one box searches whatever page is open.

## Scope

**In scope:**

- Search matches title, command, description and tags; every word must match.
- Topic filters (All, Monitor Lizard, Storage, Developer tools) and a Commands/Glossary switch.
- Detail pane: command in a code block, a Copy button, description, example output, safety chip, where it appears in the dashboard, and how to undo.
- Copy uses `navigator.clipboard.writeText` with a fallback that selects the text.
- Press `/` to focus the search box when no field has focus.

**Out of scope** (name the todo that covers it):

- Running commands from the page: not planned. Links from other pages into a guide entry: later.

## Target files

| File | Change |
|---|---|
| `web/index.html` | nav entry, `guide()` view, `FILTERABLE` entry |
| `src/guide.rs` | search rules shared with the page; Developer tools topic |
| `tests/fixtures/`, `tests/web/` | shared table and results, search and Copy tests |

## Steps

- [x] Add `'guide'` to `NAV` and `FILTERABLE`.
- [x] Fetch `/api/guide` once; filter in the browser.
- [x] Test in a real browser with keyboard only.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No results | "Nothing matches that" and a clear-filters button |
| Selected entry filtered out | First remaining result is selected |
| Clipboard blocked | Text is selected and the button says "Press Cmd+C" |
| Empty search | All entries, grouped by topic |
| 400 px width | List above the detail pane, both full width |

## Acceptance criteria

- Typing "snapshots" shows the Time Machine entries first and no unrelated ones.
- Every change-making command shows its safety chip in the list and in the detail pane.

## Tests

Search, ranking, URL-state and Copy-flow logic tests in Node, with a shared table of expected results that `cargo test` also checks (see the Implementation Record for the counts). Layout and keyboard use checked by hand in a real browser and recorded.

## Verification

```sh
make check
make test
cargo run --release -- serve   # open Guide, search "docker", copy a command
```

## Docs to update

Plan status; README page list.

## Risks and open questions

- Clipboard access inside the Tauri webview (A17) may differ; check there.
- Open: the weak-match cutoff lives only in the page. `help` lists every match, best first.
- Open: no screen reader was used, so the live-region wording is untested by ear.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** The Guide is a nav item in `web/index.html`. It fetches `/api/guide` once, on the first visit, and filters in the browser. A Commands and Glossary switch and topic chips with counts narrow the list. The detail pane shows the command, Copy, safety chip, example output, where it appears and how to undo.

**Search rules.** Every word must match title, command, tags or description. A word with no letter or digit is dropped, so `/` is an empty query. A trailing "s" on a word longer than three letters is cut. A word under three characters must stand alone in the title, command or tags. Title hits score 8, tags 4, command 4 and description 1.

**Weak matches.** Results scoring under 75% of the best score are hidden behind "N weaker matches hidden" and a Show them button. At 60% the app's own snapshot entry (12 of 17) stayed in the list, so the cutoff is 0.75. Chip counts and the cutoff use the best score over all topics.

**Real-table results.** With the cutoff, the first entry for each query was the one a user expects. Shown / hidden counts: snapshots 2/2, snapshot 2/2, docker 2/0, prune 1/0, sudo 2/2, simulators 1/0, free space 1/1, df 1/1, delete 2/0, brew 1/0, Time Machine 1/1, privilege 1/1. "Time Machine" hides the delete entry, and "df" hides the Docker one.

**Shared rules.** `help` in `src/guide.rs` follows the same rules, minus the cutoff. `tests/fixtures/guide-table.json` is the table, `cargo test` fails if it is stale, and `tests/fixtures/guide-search.json` holds the ordered ids for 22 queries that both Rust and the web tests check. The Docker, Xcode and Homebrew entries moved to a `developer` topic.

**URL state.** The Guide uses the existing hash code with keys `q`, `topic`, `tab` and `entry`. A `PAGE_PARAMS` list checks each value. Choosing an entry adds a history entry; search, topic and tab replace it. A Back or pasted link clears a stale Copy result.

**Polls and focus.** Polls refresh badges as before. The Guide's content is rebuilt only when its own inputs change, so focus and the selected command survive a poll. One live region outside the rebuilt page announces result counts and the Copy result.

**Deviations.** `/` works on the Guide only. The state names `ex` and `gsel` were kept, to avoid a wide rename. Test counts: 60 web tests and 143 Rust tests passed at the last run.

**Checked in Chrome** (own tab, port 4964, keyboard). `/`, typing "snapshots", Tab to Show them (focus landed on the first revealed row), a row, Copy (real clipboard worked), Glossary and back (the Copy label reset). A Persistence badge appeared while a Guide row had focus, and focus stayed. Searches `docker`, `a` and `/` gave 2, 2 and 21 commands. Dark and light looked right, and at 400 px in light the list sat above the detail pane with no horizontal scroll.

**Pending.** The 400 px layout in dark after this round, a simulated blocked clipboard after this round (covered by unit tests only), the Tauri webview's clipboard (A17), and any screen reader pass. The live-region wording has not been heard.
