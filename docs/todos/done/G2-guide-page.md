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

Search logic tests in the simulated DOM harness, 6 cases. Layout and keyboard use checked by hand in a real browser and recorded.

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

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

**Built.** The Guide is a nav item in `web/index.html`. It fetches `/api/guide` once, on the first visit, and filters in the browser. A Commands and Glossary switch and topic chips with counts narrow the list. The detail pane shows the command, Copy, safety chip, example output, where it appears and how to undo.

**Search.** Every word must match title, command, tags or description. A trailing "s" on a word longer than three letters is dropped, so "snapshots" finds "snapshot". Title, tag and command hits rank above description hits. An empty search groups entries by topic.

**URL state.** The Guide uses the existing hash code with new keys: `q` for search, `topic`, `tab` and `entry`. A new `PAGE_PARAMS` list checks each value. Choosing an entry adds a history entry; search, topic and tab replace the current one. New URL tests cover the keys, their allow-lists and the history rules.

**Deviations.** The topic chips come from the table, so there is no "Developer tools" chip: the Docker and Xcode commands sit under Storage. Chip labels come from the topic text, so the binary name is not in the JavaScript. The `/` shortcut works on the Guide only. Searching "snapshots" also lists the two entries about the app's own snapshot, after the Time Machine entries, because they match the word.

**Checked in Chrome, keyboard.** Own tab, dark and light, port 4962. `/`, typing, Tab to a topic chip, a row, Copy, Clear filters and Glossary all worked, with focus kept on the control used. "snapshots" and "docker" gave the expected lists. Copy worked on the real clipboard.

**Checked in Chrome, other states.** A blocked clipboard was simulated by making `writeText` reject. The command was selected, the button read "Press Cmd+C", and the selection survived a 10 s poll. The empty state showed "Nothing matches that" with Clear filters. At 400 px in an iframe the list sat above the detail pane, with no horizontal scroll. The safety chip was clipped there until fixed.

**After merging main.** One more look on port 4963 showed the Guide with the new product name in the topic chip.

**Pending.** The Tauri webview's clipboard (A17) and a screen reader pass.
