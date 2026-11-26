# G2: Guide page

| Field | Value |
|---|---|
| ID | `G2` |
| Status | `ready` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D8, D9 |
| Depends on | G1 |
| ADRs | None |
| Branch | `feat/g2-guide-page` |
| PR title | `feat(web): add the searchable Guide page` |
| Size | M (200-400) |
| PR | — |

## Goal

Add a Guide nav item with a search box, topic filters, a Commands list with a detail pane, and a Glossary, matching the design board "E · Command guide".

## Context

- G1 serves `GET /api/guide`.
- The dashboard's existing search box (`S.q`, `FILTERABLE`) can feed the Guide, so one box searches whatever page is open.

## Scope

**In scope:**

- Search matches title, command, description and tags; every word must match.
- Topic filters (All, Sys Monitor, Storage, Developer tools) and a Commands/Glossary switch.
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

- [ ] Add `'guide'` to `NAV` and `FILTERABLE`.
- [ ] Fetch `/api/guide` once; filter in the browser.
- [ ] Test in a real browser with keyboard only.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
