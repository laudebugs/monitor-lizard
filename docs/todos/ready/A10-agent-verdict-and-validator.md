# A10: Clamp the verdict to the rules

| Field | Value |
|---|---|
| ID | `A10` |
| Status | `ready` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A8 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a10-agent-verdict-and-validator` |
| PR title | `feat(classify): validate the verdict and clamp it to the rule label and allowed actions` |
| Size | S (under 200) |
| PR | — |

## Goal

A pure Rust function that turns a raw verdict into a safe one: the label can only escalate beyond the rule label, and every action must be in the allowed set. A wrong or injected reply then cannot exceed what rules allow.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Plan decision D4: the model can escalate toward caution, never downgrade a rule's "suspicious".

## Scope

**In scope:**

- `validate_verdict(rule_label, allowed, verdict) -> Verdict` in `src/classify.rs`.

**Out of scope** (name the todo that covers it):

- Showing the verdict: A15.

## Target files

| File | Change |
|---|---|
| `src/classify.rs` | `Verdict`, `validate_verdict` |

## Steps

- [ ] Write the table below as tests first.
- [ ] Implement; make A8's result pass through it before anything else sees it.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Model says benign for a rule label of suspicious | Clamped to suspicious |
| Model says suspicious for a trusted rule label | Kept: escalation is allowed |
| Action outside the allowed set | Dropped; if none remain, `investigate` |
| Rationale over 240 characters or with control characters | Truncated and cleaned |
| Rationale contains markup | Left as text; the UI escapes it (A15) |
| A8 returned `Unavailable` or `Invalid` | Rule label only, `investigate` as the sole action |

## Acceptance criteria

- No test can produce a verdict with an action outside the allowed set or a label weaker than the rule.

## Tests

Rust tests for every row (6 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- None known.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
