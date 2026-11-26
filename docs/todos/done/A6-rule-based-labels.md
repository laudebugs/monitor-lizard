# A6: Rule-based trust labels

| Field | Value |
|---|---|
| ID | `A6` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a6-rule-based-labels` |
| PR title | `feat(rules): label every app group trusted, unknown or suspicious from rules` |
| Size | M (200-400) |
| PR | [#9](https://github.com/laudebugs/system-monitor/pull/9) |

## Goal

Give every app group a deterministic label and the list of rules that produced it, and show both in the Apps table and drawer. This is the first step of the classifier plan: the label is decided by rules, never by a model ([local classifier research](../../research/local-classifier/README.md)).

## Context

- Plan decision D4: rules decide the label.
- Today `rules.rs` returns findings only; the dashboard derives a status in JavaScript (`status()` in `web/index.html`).

## Scope

**In scope:**

- `src/rules.rs`: `label_of(group, findings) -> (Label, Vec<&'static str>)`; add `label` and `rules_fired` to the `Group` JSON.
- Apps table: a Label column with a chip; drawer: "Rules fired" list.

**Out of scope** (name the todo that covers it):

- Any model or model-written rationale: later todos.
- User feedback ("mark as expected"): later todo.

## Target files

| File | Change |
|---|---|
| `src/model.rs` | `Label` enum; `label` and `rules_fired` on `Group` |
| `src/rules.rs` | `label_of` plus table-driven tests |
| `src/snapshot.rs` | set labels after findings |
| `web/index.html` | Label column, drawer section, replace JS `status()` where it duplicates the rule |

## Steps

- [ ] Define the label table below in code and tests first.
- [ ] Add `rules_fired` ids: `unsigned-network`, `exposed-listener`, `debug-port`, `unreviewed`, `new-remote`, `apple-signed`, `approved`.
- [ ] Expose in `/api/snapshot`; update the UI.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Unsigned binary with non-loopback sockets, approved or not | suspicious |
| Not approved, not Apple, non-loopback sockets | unknown |
| Approved with an `exposed-listener` or `new-remote` finding | unknown |
| Approved, no findings | trusted |
| Apple-signed, no high finding | trusted |
| No sockets at all | trusted only if Apple-signed or approved, else unknown |

## Acceptance criteria

- Every group in `/api/snapshot` has a label and non-empty `rules_fired` unless trivially trusted.
- No label is ever produced by anything but `label_of`.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

Table-driven Rust tests for every row above, 8 cases minimum, including a regression test that an unsigned network app stays suspicious after approval.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2
- [local classifier research](../../research/local-classifier/README.md) if the schema changes

## Risks and open questions

- The label table is opinion; the owner may want to change it. Keep it in one function so changing it is one edit.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `Label` (trusted, unknown, suspicious) in `src/model.rs`; `rules::classify(group, findings) -> (Label, rules_fired)` in `src/rules.rs`, computed from the same findings the dashboard shows, so the label and the findings cannot drift apart; `snapshot::take` fills it after all findings exist (including new-remote). `Group` gains `trust` and `rules_fired`, `Finding` gains `group_key`. The Apps table has a Label column (colored tag, tooltip of the rules) and the detail panel a "Label: ... (decided by rules)" section that explains each rule in words.
- **Deviations from the sketch:** the field is `trust`, not `label`, because `Group.label` already holds the app's display name. A new rule id `unverified` names the reason for an unknown label when no finding does (a group that is not approved and not known to be Apple's). Approving an app with any finding other than a clean approval makes it unknown again; an unsigned program using the network stays suspicious even when approved (regression test).
- **Tests:** 24 Rust tests pass (15 before; 9 new, one per row of the label table, each calling one small helper rather than a literal table: unsigned network approved or not, unapproved signed third party, approved with an exposed listener or a new host, approved with a debug port, approved and clean, Apple-signed with LAN exposure, no sockets, Apple-signed with a debug port staying trusted, and findings of other groups not leaking into a label).
- **Decision:** an Apple-signed group stays trusted even with a medium finding such as an open debug port (tested); revisit if an Apple tool ever opens one.
- **Real run on this Mac:** 403 groups; every group has `trust`, and every group that is not trusted has a stated reason. Of the 17 groups using the network, the 8 Apple ones are trusted and the 9 third-party ones unknown (for example Chrome: `exposed-listener`, `unreviewed`, `debug-port`). No group is suspicious today. The other 386 groups have no sockets and are unknown because signing is only checked for groups with sockets; the Apps page lists only groups with sockets, so this does not show.
- **Page check:** a script (kept out of the repo) rendered the Apps page in a simulated DOM against a running server: the Label column is present, every row's label matches the server (rows whose display names are not unique excluded: two builds of this app are both named ownwatch), the Chrome and Notes drawers explain their rules in words, no script errors.
- **Not verified:** layout in a real browser (none connected), so the visual check is **pending**; `/code-review` and `/writing-review` have not been run on this change (pending, per `AGENTS.md`).
- **Kept on purpose:** the page's `status()` (approved, macOS, review, idle) still drives the Status column, the filter chips and the counts. It is the review workflow ("does this need a decision?"), not a trust verdict, and replacing it would change the filters. The cost is that a row can show Status approved next to Label unknown; whether to merge the two is open.
- **Open:** the label table is opinion and lives in one function. Idle groups stay unknown until signing is checked for them.
- **Review (2026-11-26):** `/code-review high 9` ran a standards and a spec review. Fixed: a comment in `classify` described an exception the code does not make; the old "Trust" column is now "Signing" so it no longer sits next to "Label"; `classify` computes the label first and adds the `unverified` reason afterwards; the plan's section 2 now mentions the label; this record now says `status()` was kept and why, and the Apple-with-debug-port decision. Not changed, as judgement calls: rule ids stay string literals (a `Rule` enum would make typos compile errors and RULE_TEXT exhaustive: follow-up); `Group` is still built by hand in three places (a constructor would absorb new fields: follow-up); `Label` keeps its ordering because the verdict clamp (A10) compares labels. The research report's model vocabulary (`benign`) differs from the rule label (`trusted`); the classifier design now uses `trusted` for both. `/writing-review` on this prose was not run (pending).
- **CI:** green before review fixes; re-run pending.
