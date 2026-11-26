# 0003. The classifier loop lives in Rust; buildanagent is used for evals

- **Status:** Accepted (owner, 2026-11-26)
- **Date:** 2026-11-26
- **Plan:** [system-monitor.md](../plans/system-monitor.md), D7

This replaces the earlier Proposed version of this record (a Node sidecar on the buildanagent SDK), which was never accepted.

## Context

The owner wants the classifier agent built with their buildanagent tooling. The [adoption research](../research/buildanagent-adoption/README.md) found the SDK and gateway can drive a local llama-server, but also that a one-shot classification does not need an agent loop: the guide's own advice is that one bounded structured call beats an agent unless the agent wins on pass^k. llama-server can enforce a JSON schema natively, which is more reliable for a 4B model than relying on it to call tools. A Node sidecar would add a runtime users must install, weekly alpha releases to track, and a second process to supervise.

## Decision

- Rust owns the classifier: it builds the evidence for one group, makes one bounded call to a local llama-server (OpenAI-compatible `/v1/chat/completions`, JSON-schema response format, temperature 0), validates the reply, and clamps it to the rules ([D4](../plans/system-monitor.md)). The model has no tools and no way to act.
- Attacker-controlled strings (process names, command lines) go into the prompt only as JSON string values in a data block, never in instructions.
- No Node at runtime. The shipped product stays one binary and makes no network calls except to `127.0.0.1` and an explicit, user-started model download.
- buildanagent is used at development time only: the eval harness, prompt iteration and recorded-response replay (todo A16). It is not a runtime dependency.

## Consequences

- We write and test the (small) call loop ourselves; the tests mirror production because they run the real Rust code against a fixture server.
- Todos A8 (structured call), A10 (validator), A13 (supervise llama-server) and A16 (evals) follow; the earlier sidecar, read-only-tools and Node-packaging todos are removed.
- If the agent later needs multi-step tools, sessions or approvals, supersede this record and revisit the Node sidecar option in the research.
- It is still unverified that llama-server's `response_format` JSON schema works with the chosen model; A8 verifies it against the real server.

## Alternatives considered

- Node sidecar on `@buildanagent/sdk` and `gateway`: typed loop and test model, but a Node runtime on users' machines, alpha churn and a second process.
- `buildanagent` CLI or harness as the agent: the CLI cannot target a local URL; the harness brings shell, file and network tools.
- Harness JSON-RPC to the dashboard: heavy for stateless classification and creates a session log of process names.
