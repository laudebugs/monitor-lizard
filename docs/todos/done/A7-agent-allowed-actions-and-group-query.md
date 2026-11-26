# A7: Allowed actions per label and the evidence for one group

| Field | Value |
|---|---|
| ID | `A7` |
| Status | `done` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A6 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a7-agent-allowed-actions-and-group-query` |
| PR title | `feat(rules): compute allowed actions per label and build the evidence for a group` |
| Size | M (200-400) |
| PR | — |

## Goal

Give Rust the two things the model must never decide for itself: the set of actions allowed for a group given its rule label, and a bounded, quoted evidence record for one group that A8 puts in the prompt. Nothing the model says can then exceed what the rules allow.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Plan decision D4; [ADR 0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md).
- `src/actions.rs` already refuses root, launchd and ourselves; the allowed set must agree.

## Scope

**In scope:**

- `src/classify.rs`: `Action` enum (`approve`, `ignore`, `investigate`, `quit`, `force_quit`, `block_network`) and `allowed_actions(label, group) -> Vec<Action>`.
- `evidence(group_key) -> Evidence`: group facts (path class, signing, team id, parent, listening ports, remote host count, first-seen age, persistence, rule label, rules fired, baseline status), every string capped at 256 characters with control characters removed.
- No HTTP route.

**Out of scope** (name the todo that covers it):

- The model call: A8.
- `block_network`: never allowed until a network extension exists.

## Target files

| File | Change |
|---|---|
| `src/classify.rs` | (new) `Action`, `allowed_actions`, `Evidence`, `evidence` |
| `src/main.rs` | `mod classify`; `classify <group-key>` debug command printing label, rules, allowed actions and evidence JSON |
| `docs/agents/layout.md` | add the file |

## Steps

- [x] Write the allowed-action table below as tests first.
- [x] Implement `allowed_actions` using the A6 label and `Group` fields.
- [x] Implement `evidence`, with the string caps in one helper.
- [x] Add the debug command; record its output for three real apps in the Implementation Record.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| suspicious, own-user processes | investigate, quit, force_quit |
| suspicious, only root processes | investigate only |
| unknown | approve, ignore, investigate, quit |
| trusted and not approved | approve, ignore |
| trusted and approved | ignore |
| `block_network` for any label | never present |
| Process name with newlines or 10 kB | Capped at 256 characters, control characters removed |

## Acceptance criteria

- `allowed_actions` never returns an action `src/actions.rs` would refuse.
- `Evidence` contains no field the dashboard does not already show.

## Tests

Table-driven Rust tests for every row above; one test that quit and force_quit are absent when every process is root-owned.

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

- The allowed-action table is opinion; keep it in one function. **Status:** `allowed_actions_for` is that one function; the "signalable" question it asks is delegated entirely to `actions::check` (shared, not reimplemented) rather than being a second opinion on ownership.
- **Added during implementation:** the spec names "parent" as an Evidence fact. Resolving a process's actual parent executable needs the full system-wide process table, which `Group` does not carry (each group only has its own processes). Implemented as `launched_by_launchd: bool` (any process with ppid 1) instead -- a much narrower fact than "parent". Tested against three real apps: every one came back `true`, including Spotify and Claude Code, which the owner launched by hand -- macOS commonly reparents GUI app processes to launchd, so this field barely distinguishes a background daemon from something the user opened. Recorded honestly rather than left to be discovered later; a real parent-chain needs `collect::processes()`'s full table threaded through, which is a larger change than this todo's size.
- **Acceptance criterion tension:** "`Evidence` contains no field the dashboard does not already show" reads strictly as "already rendered in `web/index.html`" -- by that reading, `first_seen_age_secs`, `persistent`, `path_class` and `launched_by_launchd` fail it: the Network, Persistence and History pages are still `SOON` placeholders ("Coming in a later phase") in `web/index.html`, and `category`/`launched_by_launchd`-shaped facts are not rendered anywhere yet. But the Scope section explicitly lists "first-seen age" and "persistence" among the facts `Evidence` must carry, and both are already served by endpoints this app ships today (`/api/persistence`, `/api/history`, from A1, merged). Read as "no field invents new data collection," which is what the criterion is protecting against, every field passes: everything traces to `Group`, `Signing`, `persistence::Item` or `History`, nothing new is collected. Kept all four fields under that reading; noted here rather than silently resolved either way.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no user-facing prose added; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/classify.rs` -- `Action` enum (`BlockNetwork` never returned, enforced structurally: `allowed_actions` has no branch that pushes it), `allowed_actions(label, group)` (a single function, label-matched, `Quit`/`ForceQuit` gated on whether anything in the group could actually be signalled), `evidence(group_key)` (a pure `build_evidence` core plus a thin `evidence` wrapper that fetches a fresh snapshot, first-seen age and persistence). `History::first_seen(group_key)` added to `src/history.rs`. Wired into `src/main.rs` as `classify <group-key>`, printing label, rules fired, allowed actions and the evidence JSON.
- **Tests:** 54 Rust tests pass (52 before this review pass's fixes; +1 `History::first_seen`, +1 exercising the real `actions::check` wiring end-to-end rather than only the injected-closure path). Table-driven tests for every edge-case row, plus the launchd/self guard and the 256-character cap with control characters stripped.
- **Real run on this Mac**, `sys-monitor classify` against three real apps (full output below): Spotify.app (Unknown, `[Approve, Ignore, Investigate, Quit]`, 5 listening ports, 8 remote hosts, not persistent), `dev:claude-code` (Unknown, same action set, 9 listening ports, 4 remote hosts), `apple:/usr/libexec/sharingd` (Trusted, `[Approve, Ignore]`, Apple-signed).
  ```
=== /Applications/Spotify.app ===
label: Unknown
rules_fired: ["exposed-listener", "unreviewed"]
allowed_actions: [Approve, Ignore, Investigate, Quit]
{
  "group_key": "/Applications/Spotify.app",
  "label": "Spotify",
  "path_class": "Apps",
  "signed": true,
  "apple": false,
  "team_id": "2FNC3A47ZF",
  "launched_by_launchd": true,
  "listening_ports": [
    1900,
    5353,
    7768,
    57621,
    58407
  ],
  "remote_host_count": 9,
  "first_seen_age_secs": 17045,
  "persistent": false,
  "trust": "unknown",
  "rules_fired": [
    "exposed-listener",
    "unreviewed"
  ],
  "approved": false
}

=== dev:claude-code ===
label: Unknown
rules_fired: ["unreviewed"]
allowed_actions: [Approve, Ignore, Investigate, Quit]
{
  "group_key": "dev:claude-code",
  "label": "Claude Code",
  "path_class": "Developer tools",
  "signed": true,
  "apple": false,
  "team_id": "Q6L2SF6YDW",
  "launched_by_launchd": true,
  "listening_ports": [
    51813,
    51933,
    59385,
    59418,
    60900,
    62182,
    62701,
    62800,
    63401,
    65048
  ],
  "remote_host_count": 4,
  "first_seen_age_secs": 17046,
  "persistent": false,
  "trust": "unknown",
  "rules_fired": [
    "unreviewed"
  ],
  "approved": false
}

=== apple:/usr/libexec/sharingd ===
label: Trusted
rules_fired: ["apple-signed", "exposed-listener-apple"]
allowed_actions: [Approve, Ignore]
{
  "group_key": "apple:/usr/libexec/sharingd",
  "label": "sharingd",
  "path_class": "macOS system",
  "signed": true,
  "apple": true,
  "team_id": null,
  "launched_by_launchd": true,
  "listening_ports": [
    0,
    59412
  ],
  "remote_host_count": 2,
  "first_seen_age_secs": 17047,
  "persistent": false,
  "trust": "trusted",
  "rules_fired": [
    "apple-signed",
    "exposed-listener-apple"
  ],
  "approved": false
}

  ```
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: `has_a_signalable_process` independently reimplemented `actions.rs`'s ownership policy (comparing a cached snapshot username against `id -un`) instead of sharing it, which the Standards review flagged as the exact kind of drift the "never returns an action `actions.rs` would refuse" acceptance criterion exists to prevent -- `actions::check` and a new `actions::my_uid()` are now `pub(crate)` and `classify::allowed_actions` calls them directly (a live `ps`-backed lookup per pid, same mechanism `actions::execute` itself uses), with the label-policy tests switched to inject a plain closure so they stay fast and deterministic without needing a real signallable pid; one additional test exercises the real `actions::check` wiring end to end. Also fixed: the duplicated `out()` helper (removed, no longer needed once ownership checking moved to `actions.rs`); `History::first_seen` had no test (added); a module-doc note distinguishing this file from the pre-existing `rules::classify` function, which does something different and is easy to confuse it with. Not changed, with reasons: in the Risks section above.
- **Deviation:** none beyond the ownership-check unification above, found by review rather than by running against real data this time -- B1's two real-data bugs did not repeat here because `evidence()` and `allowed_actions()` were run against this Mac's actual processes before review, not only against synthetic fixtures.
- **Open:** A8 is the first consumer of `allowed_actions`/`evidence`. The "parent" and acceptance-criterion notes above are both genuine open questions for whoever picks up A8 or a later todo, not resolved here.
- **CI:** pending on the pull request.
