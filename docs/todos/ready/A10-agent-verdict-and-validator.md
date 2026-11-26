# A10: Verdict schema and the Rust validator

| Field | Value |
|---|---|
| ID | `A10` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A9 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `feat/a10-agent-verdict-and-validator` |
| PR title | `feat(agent): add the verdict output tool and clamp it in Rust` |
| Size | M (200-400) |
| PR | — |

## Goal

The agent ends with a schema-checked verdict; Rust re-validates it, clamps the label to the rule label and drops any action outside the allowed set, so a wrong or injected verdict cannot exceed what rules allow.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Plan decision D4: the model can escalate toward caution, never downgrade a rule's "suspicious".
- Schema from the local classifier research: `label`, `confidence`, `actions`, `rationale` (240 characters at most).

## Scope

**In scope:**

- `outputTool` with a flat schema and enums.
- `validate_verdict(rule_label, allowed, raw) -> Verdict` in Rust.

**Out of scope** (name the todo that covers it):

- Showing the verdict in the UI: A15.

## Target files

| File | Change |
|---|---|
| `agent/src/verdict.mjs` | (new) schema and `outputTool` |
| `src/classify.rs` | `Verdict`, `validate_verdict` |
| `agent/test/verdict.test.mjs` | (new) |

## Steps

- [ ] Write the Rust validator tests first.
- [ ] Implement the schema and tool; make the agent end with it.
- [ ] Add a replay test with a recorded response (`replayPairs`).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Model says benign for a rule label of suspicious | Clamped to suspicious |
| Model says suspicious for a trusted rule label | Kept: escalation is allowed |
| Action outside the allowed set | Dropped; if none remain, `investigate` |
| Rationale over 240 characters or containing control characters | Truncated and cleaned |
| Output is not valid per schema after retries | Fall back to the rule label, no actions beyond `investigate` |

## Acceptance criteria

- No test can produce a verdict with an action outside the allowed set or a label weaker than the rule.

## Tests

Rust tests for every row (6 minimum) and Node tests for the schema.

## Verification

```sh
make check
make test
(cd agent && npm test)
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- Whether strict schemas and grammar pass through llama-server is unverified; test against the real server in A12.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
