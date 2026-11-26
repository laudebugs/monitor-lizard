# A6: Rule-based trust labels

| Field | Value |
|---|---|
| ID | `A6` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a6-rule-based-labels` |
| PR title | `feat(rules): label every app group trusted, unknown or suspicious from rules` |
| Size | M (200-400) |
| PR | — |

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
make check\nmake test
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2
- [local classifier research](../../research/local-classifier/README.md) if the schema changes

## Risks and open questions

- The label table is opinion; the owner may want to change it. Keep it in one function so changing it is one edit.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
