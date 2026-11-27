# R1: Overview redesign

| Field | Value |
|---|---|
| ID | `R1` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11, D12 |
| Depends on | T1b, N1, A9 |
| ADRs | None |
| Branch | `feat/r1-overview-redesign` |
| PR title | `feat(web): redesign Overview as a list of decisions with evidence` |
| Size | M (200-400) |
| PR | pending |

## Goal

Replace the Overview with the layout on design board "F · Overview". It has a one-sentence verdict as the title, count tiles that link to pre-filtered pages, a "Needs a decision" list with evidence and the rule that fired. It also has a "Since you last looked" feed, a Machine strip with links, and outbound sockets by app.

## Context

- Today's `overview()` shows four tiles, an attention list from `cards()` and top talkers.
- The verdict and feed need a "last looked" time: store it in the browser (`localStorage`) and fall back to 24 hours.

## Scope

**In scope:**

- Title text computed from the number of items needing a decision ("2 things need a decision", "Nothing needs a decision").
- Each decision shows severity, title, why, evidence (paths, hosts, ports), the rule id, a link to the item's page via N1, and two actions (primary and secondary) that already exist or come from A9.
- Feed rows: new hosts, new startup items, first outbound for an app, sudo launches since the last visit; each row links to the item.
- Machine strip: free space and forecast (S4 when it exists, otherwise free space only), memory pressure and CPU from `/api/metrics`; each row links to Performance or Storage and shows "unavailable" with a reason when the source is missing.

**Out of scope** (name the todo that covers it):

- Muting for a time: later. The Machine strip's forecast: S4.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `overview()` and its helpers |
| `src/rules.rs` | expose the rule id on each finding if not already in the JSON |

## Steps

- [ ] Check `/api/snapshot` findings already carry a rule id; add it if not, with a test.
- [ ] Build the feed from `S.seen`, `S.pers` and `S.sudo` filtered by the last-looked time.
- [ ] Check in a real browser at 1360 px and 400 px.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No decisions | "Nothing needs a decision" and the feed still shows |
| First ever visit | Feed covers the last 24 hours and says so |
| Metrics not available | Machine rows say "unavailable" with the reason, never 0 |
| 200 decisions | List paginates with the existing pager |
| Long evidence strings | Wrap inside the row; no horizontal scroll |

## Acceptance criteria

- Every number, row and evidence line on the page links to where it came from.
- Marking an alert expected removes it from the list without a reload.

## Tests

Logic tests for the title text and the feed filter, 8 minimum. Layout checked by hand and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

README screenshots or page descriptions; plan status.

## Risks and open questions

- Machine strip depends on pages (Performance, Storage) that may not exist yet; hide a row until its page ships.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Title:** "N things need a decision" or "Nothing needs a decision". The count is the number of rows in the decisions list.
- **Decisions:** each row has a severity, a title that links to the item, why, evidence (paths, hosts, ports), the rule text with its id, and at most two actions. The Details buttons became the title link. `/api/snapshot` findings already carried `rule` and `evidence`, so `src/rules.rs` did not change. Rows that do not come from a finding use the ids `unreviewed`, `failed-sudo` and `new-startup-item`, which `RULE_TEXT` now explains. The list paginates with the shared pager.
- **Feed:** new hosts, apps whose first recorded host is newer than the last visit, new startup items and sudo launches. Each row links to the item. The last visit is a `localStorage` time written when the page is hidden or closed; a first visit covers 24 hours and says so.
- **Machine strip:** free space, the capacity forecast (when the history has one), memory pressure and CPU. Each row links to Storage or Performance and shows "Unavailable: reason" when its source is missing. The Overview now polls metrics and volume, which only the Performance page did before.
- **Tests:** `tests/web/overview-page.test.mjs` (9 cases), and the title text in `page-shell.test.mjs`. `make check` and `make test` pass.
- **Real browser (Chrome, scratch HOME, release build):** at 919 px the page rendered the title, the tiles, the decisions with evidence and rule, the feed, the Machine strip and the sockets list, with no horizontal scroll. In a 400 px frame the page was 396 px wide with nothing past the edge. Marking a finding expected took the count from 6 to 5 with no reload. CPU read 100% then 62%, matching `/api/metrics`.
- **Open:** the layout was not compared with the "F · Overview" board; the owner's design file was not available. 1360 px was not available (window limit). The feed on a fresh history lists every host as new, because every host is first seen after the database starts.
