# A5: History page

| Field | Value |
|---|---|
| ID | `A5` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a5-history-page` |
| PR title | `feat(web): add the history page` |
| Size | S (under 200) |
| PR | — |

## Goal

Replace the History placeholder with what changed over time: hosts first seen per app, and an audit table of every Stop/Quit the dashboard performed.

## Context

- `/api/history` returns `seen` and `actions` ([A1](../done/A1-backend-persistence-history-api.md)).
- Plan decision D3 requires the audit log to be visible to the user.

## Scope

**In scope:**

- Two tables: "New hosts" (when, app, host, hits) and "Actions you took" (when, signal, pids sent, refused).

**Out of scope** (name the todo that covers it):

- Retention and deletion of history: not covered.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove `history` from `SOON`; add `history()` view with `paginate(..., "hist")` and `paginate(..., "act")` |

## Steps

- [x] Map `group_key` to the app label using the current snapshot; fall back to the key's basename.
- [x] Render `refused` JSON as short reasons.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Empty history | Empty-state rows |
| `group_key` for an app no longer running | Basename shown |
| `sent` empty and `refused` non-empty | Row shows the refusal reason |

## Acceptance criteria

- An action performed through the dashboard appears here within one refresh.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

jsdom smoke test as in A2.

## Verification

```sh
make check\nmake test
```

## Docs to update

- Plan section 2

## Risks and open questions

- None known.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose beyond short table/empty-state copy following established patterns; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `history()` view in `web/index.html` -- two tables, "New hosts" (When, App, Host, Hits) and "Actions you took" (When, Signal, Sent, Refused), both sortable (`sortRows`/`sortHeader`, matching every sibling page), both defaulting to newest-first, both filterable by the global search box. `labelOf(key)` resolves a group_key to its current app label from the live snapshot, falling back to a basename with the `apple:`/`dev:`/`other:` prefix stripped via a new shared `unprefixed()` helper (also now used by `drawer()`, which had the same regex inline). `jsonOr()` safely parses the `sent`/`refused` JSON-string fields the backend returns. `loadHist()` (added in A4) now also fetches `actions`, not just `seen`. Since every nav page is now built, the now-empty `SOON` object and its unreachable placeholder-rendering branch were removed rather than left as dead code.
- **Real run on this Mac:** 869 real "seen" rows and 5 real actions, including one genuine refusal ("not your process") and two "refusing to signal launchd" refusals from earlier in this session -- all rendered with their real reason text, not a raw count or undigested JSON. Apps no longer running (`akd`, `gh`, `cloudd`, `(ssh)`, `amsaccountsd`) correctly showed their basenames, not raw `apple:`/`dev:`-prefixed keys.
- **Real-browser verification:** no Claude-in-Chrome extension was connected this session, so headless Chrome was driven directly over the DevTools Protocol. Checked: both tables render against real data; a refused action's real reason text appears on the page; no raw group_key prefix leaks into the rendered table; search filters in place without jumping views; clicking a column header (Hits) actually re-sorts the rows; searching for a term that matches no action shows the "no actions match the search" message with a working Clear filters button, not the misleadingly-worded always-empty message the first draft had. Layout checked visually at 1440 px and 1000 px (below the todo's 1100 px threshold), including the full page height showing both tables and pagers (1-25 of 859+ rows, 35 pages).
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: `stop()` refreshed the snapshot after a kill (`setTimeout(() => load(true), 700)`) but never refreshed history, so an action the user just took would not appear on this page until the next ambient 30 s poll, not "within one refresh" as the acceptance criterion requires -- added a matching `setTimeout(loadHist, 700)`; both tables' empty states collapsed "truly empty" and "filtered to zero" into one message with no way to clear the filter, unlike every sibling page -- rewritten to match the established `all.length ? '...matches...' + clearBtn() : '...recorded yet'` pattern, catching a real bug on the Actions table (it always showed the same message regardless of whether a search was active); neither table's columns were actually sortable despite looking identical to every other page's sortable headers -- rewritten to use real `sortRows`/`sortHeader` with the default order baked into the base array, the same convention `network()` already established; the `apple:`/`dev:`/`other:` prefix-strip regex was duplicated between the new `labelOf()` and the pre-existing `drawer()` -- extracted into one `unprefixed()` helper used by both. All fixes reverified against the real browser, including a direct check that clicking a column header actually re-sorts rows and that the filtered-to-zero message now appears correctly. Not changed, with reasons: `refusedText()` shows only the first distinct reason plus "..." for multiple reasons, flagged as a minor judgement call by review -- kept, since it matches the identical summarization already used in `stop()`'s own toast message, so it's consistent with established precedent rather than a new inconsistency.
- **Deviation:** none from the sketch beyond the review fixes above.
- **Open:** none. Retention and deletion of history are explicitly out of scope (a later todo, per Risks).
- **CI:** pending on the pull request.
