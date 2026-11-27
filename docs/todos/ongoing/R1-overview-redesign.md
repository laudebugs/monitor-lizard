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
| PR | — |

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
