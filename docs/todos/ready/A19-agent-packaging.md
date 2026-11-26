# A19: Package the Node runtime and the sidecar

| Field | Value |
|---|---|
| ID | `A19` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A13 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `build/a19-agent-packaging` |
| PR title | `build(agent): ship a pinned Node runtime and the agent bundle` |
| Size | M (200-400) |
| PR | — |

## Goal

Decide and implement how a user without Node runs the sidecar: a vendored pinned runtime or a single-file bundle, with a test asserting the sidecar makes no outbound connection.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Single-file executables and macOS sandbox-exec confinement were not verified.

## Scope

**In scope:**

- Spike both options; record the result in an ADR amendment.

**Out of scope** (name the todo that covers it):

- Notarization and releases: not planned yet.

## Target files

| File | Change |
|---|---|
| `docs/decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md` | record the outcome (new ADR if this supersedes it) |
| `Makefile, .github/workflows/ci.yml` | build step |

## Steps

- [ ] Try the vendored runtime and the single-file option; measure size and start time.
- [ ] Add the no-network assertion test.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Node missing on the machine | App runs rules only and says how to enable the agent |

## Acceptance criteria

- An end-to-end run on a machine without Node on `PATH`.

## Tests

A network assertion test around the packaged sidecar.

## Verification

```sh
make check
make test
```

## Docs to update

- ADR 0003

## Risks and open questions

- Depends on a decision the owner has not made (Node runtime vs Rust-only).

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
