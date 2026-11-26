# 0003. The classifier agent is a Node sidecar on the buildanagent SDK

- **Status:** Proposed (needs the owner's decision between this and a Rust-only agent)
- **Date:** 2026-11-26
- **Plan:** [system-monitor.md](../plans/system-monitor.md), D7

## Context

The owner wants the classifier agent built with their own buildanagent tooling. The [adoption research](../research/buildanagent-adoption/README.md) found the SDK and gateway can drive a local llama-server, the CLI cannot, the harness is heavier than needed, and there is no pass^k eval runner yet. The agent must not weaken [D4](../plans/system-monitor.md): rules decide the label and a human approves every action.

## Decision

- A Node (22.19 or later) sidecar, spawned and supervised by the Rust binary, runs `@buildanagent/sdk` and `@buildanagent/gateway` (pinned exactly, `alpha` tag) against a local llama-server through the `openaiChat` adapter.
- The sidecar has four read-only tools and one `outputTool` verdict. It has no write tools and no approval step.
- Rust computes the rule label and the allowed action set, validates the verdict, clamps the label and actions, and falls back to the rule label alone when the sidecar or model is unavailable.
- The sidecar runs with a cleared environment and no network except `127.0.0.1`.

## Consequences

- Users need Node, or we ship a pinned runtime (todo A19).
- We track weekly alpha releases of our own packages: pin and bump deliberately.
- We write the eval corpus and the pass^k runner ourselves (todo A16).
- If Node on users' machines is unacceptable, supersede this with option (c) in the research: Rust talks to llama-server directly and buildanagent is used only for evals.

## Alternatives considered

- `buildanagent` CLI or harness as the agent: the CLI cannot target a local URL; the harness brings shell, file and network tools.
- Rust-only agent: no Node dependency and the smallest attack surface, but we would write and test the tool loop ourselves.
- Harness JSON-RPC to the dashboard: heavy for stateless classification and creates a session log of process names.
