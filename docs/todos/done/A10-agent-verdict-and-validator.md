# A10: Clamp the verdict to the rules

| Field | Value |
|---|---|
| ID | `A10` |
| Status | `done` |
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

- [x] Write the table below as tests first.
- [x] Implement; make A8's result pass through it before anything else sees it.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Model says trusted for a rule label of suspicious | Clamped to suspicious |
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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/classify.rs` -- `validate_verdict(rule_label, allowed, verdict) -> Verdict` turns A8's raw, untrusted reply (or the lack of one) into a `Verdict` that can never exceed what the rules already decided. The label is clamped via `rule_label.max(raw.label)`, using `Label`'s own declared order (Trusted < Unknown < Suspicious, already `Ord`) as its severity order. Actions are rebuilt in the model's own order, keeping only those in `allowed` and dropping later repeats of one already kept (order-preserving, not `Vec::dedup`'s adjacent-only removal). `Verdict` is a new type distinct from the pre-existing `llm::Verdict`: its only constructor is this function, so nothing downstream can act on a verdict that skipped the checks. `cap`'s length-and-control-character cleaning was generalized to `cap_len(s, n)` and reused for the rationale's own 240-character cap (independent of `llm.rs`'s own partial cap on length alone, since that one can't be trusted to have also stripped control characters).
- **The Trusted/Investigate edge case:** the todo's own edge-case rows 3 and 6 both say the fallback is unconditionally `investigate`, but `allowed_actions_for` never includes `Investigate` for `Label::Trusted`. The acceptance criterion ("no action outside the allowed set") takes priority over the table's literal wording: both the error-path fallback and the empty-actions fallback only insert `Investigate` when it is itself in `allowed`, leaving the action list empty rather than inserting one the rules never allowed. Documented in a code comment and covered by two direct tests (one on each path).
- **Tests:** 116 total (106 before; +10 in `classify::tests`), one covering each table row plus extras: both escalation directions; actions outside `allowed` dropped with `investigate` filling the gap when allowed; the same case verified again generically (every surviving action is itself in `allowed`); the Trusted/no-Investigate case on both the `Ok`-but-filtered-empty path and the `Err` path directly; rationale truncated and cleaned past 240 characters/control characters; markup left unescaped; all three `LlmError` variants falling back identically; duplicate actions from the model collapsed without reordering.
- **Real run:** no automated UI exists yet to drive this (that's A15), so verified instead with a temporary, unshipped CLI subcommand added only in the local worktree (reverted before committing, never part of this diff) that ran the real pipeline end to end against this Mac's own running state: real `snapshot::take()`, real `classify::evidence`, a real (failed, no server running) `llm::classify` network call, through the real `validate_verdict`. Against a Trusted-and-approved app group (`/System/Applications/Notes.app`), got back `{label: trusted, actions: []}` -- confirming the Trusted/no-Investigate resolution holds for real data, not just the hand-built test fixtures. Against an Unknown group (`dev:claude-code`), got back `{label: unknown, actions: [investigate]}`, matching edge case row 6 exactly.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards review found three comments citing todo IDs ("A15", "A8... A15") as their justification -- the same AGENTS.md rule 9 mistake caught repeatedly elsewhere this session -- reworded to state the structural reason (a type whose only constructor is a validating function, temporarily unreferenced before a caller exists) without naming any todo; also flagged a copy-edit slip (unmatched parenthesis) in a comment, fixed. Spec review found no missing requirements and no scope creep, confirmed the separate `classify::Verdict` type and the Trusted/Investigate resolution were both deliberate and sound, and suggested one additional direct test (the `Err` path with `Investigate` not allowed) to make that resolution's coverage explicit on both branches rather than implied by one of them; added.
- **Deviation:** none from the sketch beyond the fixes above. While finalizing this todo, found and fixed stale `todos/ready/...` links for the already-merged A7/A8 and an un-updated "(done)" marker in `docs/plans/classifier-agent.md`'s own Todos line (pre-existing drift from the A7/A8 PRs, not introduced by this one, fixed in passing since this PR edits that file anyway).
- **Open:** none beyond A12/A13/A15/A16 remaining as separate todos.
- **CI:** pending on the pull request.
