# A7: Allowed actions per label and a read-only group query

| Field | Value |
|---|---|
| ID | `A7` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A6 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `feat/a7-agent-allowed-actions-and-group-query` |
| PR title | `feat(rules): compute allowed actions per label and expose read-only group queries` |
| Size | M (200-400) |
| PR | — |

## Goal

Give Rust the two things the agent must never decide for itself: the set of actions allowed for a group given its rule label, and read-only queries the sidecar can call. Nothing in the agent can then exceed what the rules allow.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Plan decision D4; [ADR 0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md).
- `src/actions.rs` already refuses root, launchd and ourselves; the allowed set must agree.

## Scope

**In scope:**

- `src/classify.rs`: `Action` enum (`approve`, `ignore`, `investigate`, `quit`, `force_quit`, `block_network`) and `allowed_actions(label, group) -> Vec<Action>`.
- Read-only functions returning JSON for `get_group`, `get_connections`, `get_baseline`, `get_history`, each taking a `group_key`.
- No HTTP route yet: the sidecar reaches these over stdio in A9.

**Out of scope** (name the todo that covers it):

- The sidecar: A8 to A10.
- `block_network`: never allowed until a network extension exists.

## Target files

| File | Change |
|---|---|
| `src/classify.rs` | (new) `Action`, `allowed_actions`, query functions |
| `src/main.rs` | `mod classify`; `classify <group-key>` debug command printing label, rules and allowed actions |
| `docs/agents/layout.md` | add the file |

## Steps

- [ ] Write the allowed-action table below as tests first.
- [ ] Implement `allowed_actions` using the A6 label and `Group` fields.
- [ ] Implement the four queries over the snapshot, baseline and history; cap string fields at 256 characters and strip control characters.
- [ ] Add the debug command and record its output for three real apps in the Implementation Record.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| suspicious, own-user processes | investigate, quit, force_quit |
| suspicious, only root processes | investigate only |
| unknown | approve, ignore, investigate, quit |
| trusted and not approved | approve, ignore |
| trusted and approved | ignore |
| `block_network` for any label | never present |
| Process name containing newlines or 10 kB | Truncated to 256 characters, control characters removed |

## Acceptance criteria

- `allowed_actions` never returns an action `src/actions.rs` would refuse.
- The queries expose no field the dashboard does not already show.

## Tests

Table-driven Rust tests for every row above; one test that quit/force_quit are absent when every process is root-owned.

## Verification

```sh
make check
make test
cargo run --release -- classify "/Applications/Spotify.app"
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2

## Risks and open questions

- The allowed-action table is opinion; keep it in one function.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
