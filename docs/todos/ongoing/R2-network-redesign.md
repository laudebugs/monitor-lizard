# R2: Network redesign

| Field | Value |
|---|---|
| ID | `R2` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9, D11, D14 |
| Depends on | T1b, N1, N3, A9 |
| ADRs | None |
| Branch | `feat/r2-network-redesign` |
| PR title | `feat(web): redesign Network with grouping, counts and a host inspector` |
| Size | M (200-400) |
| PR | pending |

## Goal

Redesign Network as on board "G · Network". It has a verdict title ("1 new host in the last hour"), filters with counts, and grouping by App, Host or Port. The table shows relative first-seen times, and an inspector for the selected host shows Related links.

## Context

- A4 (done) built grouped outbound connections with first-seen dates. Reverse DNS stays out: it tells a resolver which IPs you look up.
- The `isNew` rule is: approved app, first seen within an hour.

## Scope

**In scope:**

- Filters: All, New, Public, LAN, Unsigned, each with a live count.
- Group by App, Host or Port; Host grouping shows every app that contacted that address.
- Table columns: app, remote, ports, scope, sockets, first seen (relative, with the absolute time on hover). N3 adds column choice and export.
- Inspector: first and last seen, sockets, ports, other apps using the host, the app's trust, Related links (app, startup items, History of the host), and "Mark host as expected" from A9.
- A footer line that says hosts are shown as IP addresses and why.

**Out of scope** (name the todo that covers it):

- Hostname or ASN enrichment: needs an opt-in design (system-monitor plan section 5). Per-app bandwidth: not planned until a no-root source is verified.

## Target files

| File | Change |
|---|---|
| `web/index.html` | `network()` and helpers |

## Steps

- [ ] Reuse the grouping already in `network()`; add Host and Port groupings as pure functions with tests first.
- [ ] Add the inspector and Related links through N1 hashes.
- [ ] Check in a real browser at 1360 px and 400 px.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| IPv6 remote | Shown without brackets in the table, with brackets and port in the copy text |
| Same host used by two apps | Host grouping lists both |
| Host seen before history existed | First seen shows "before monitoring" |
| No outbound connections | Verdict "No outbound connections right now" |
| Host marked expected | Chip "expected", leaves New |

## Acceptance criteria

- The verdict count equals the number of rows under the New filter.
- Switching Group by keeps the filters and the selection when the selected host is still visible.

## Tests

Logic tests, 8 minimum. Layout checked by hand and recorded.

## Verification

```sh
make check
make test
```

## Docs to update

Plan status; the Network section of the README.

## Risks and open questions

- Grouping by Host on a busy machine can produce thousands of rows; paginate as today.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Board:** built from the owner's Network board.
- **Filters:** All, New, Public, LAN and Unsigned, each with a live count. The counts follow the current grouping, so the verdict and the New chip equal the number of rows under New in App, Host and Port grouping.
- **Group by:** App, Host or Port. Host grouping lists every app that reached the address. Port grouping adds up the sockets of every host behind a port. Switching keeps the filters and the selected host.
- **Table:** first seen is relative with the exact time on hover. A host older than the history reads "before monitoring".
- **Inspector:** first seen, sockets, ports, other apps, trust, why it is flagged, Related links (app, startup items, History of the host), Mark host as expected and Copy address. An IPv6 address is copied with brackets and the port.
- **Address:** the filter, grouping and selected host are in the address (`group` and `pick`). The old `host` link still searches.
- **Not built:** the board's connection map and its side panel of sockets by port. Neither is in this todo's scope.
- **Tests:** `tests/web/network-page.test.mjs` and three cases in `url-state.test.mjs`.
- **Checked in Chrome (scratch HOME, release build):** the chips with counts, all three groupings, selecting a host to open the inspector, and keeping the selection when switching groupings.
- **Checked at 400 px:** the page was 396 px wide with the inspector open.
- **Not verified:** the layout side by side with the board image, 1360 px (Chrome would not open a window that wide), and the New chip with a count above zero, because the scratch machine had no new host.
- **Known limits:** Host and Port rows have no inline Mark button, because one merged row can cover several apps; use the inspector. Port rows cannot be selected, and a port with many hosts lists them all in one cell. A merged row's scope reads "mixed" when its hosts differ.
