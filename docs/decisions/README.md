# Decisions (ADRs)

One file per hard-to-reverse decision. Agents must not assume a decision that is not recorded here.

- **Name:** `NNNN-kebab-title.md`, numbered in order of creation. Numbers are never reused.
- **Title** states the decision, with the reason when short.
- **Status:** `Proposed`, `Accepted (owner, YYYY-MM-DD)`, or `Superseded by NNNN`. A provisional record names the todo whose check can change it.
- **Never rewrite an accepted record.** Add a dated `Amended` bullet under the header for a small change, or write a new record and mark the old one superseded.
- Copy [TEMPLATE.md](TEMPLATE.md). Add a row below in the same PR.

| ADR | Decision | Status | Plan |
|---|---|---|---|
| [0001](0001-record-decisions-as-adrs.md) | Record decisions as ADRs | Accepted | n/a |
| [0002](0002-rust-core-polling-collectors-local-web-ui.md) | Rust core with polling collectors and a loopback web dashboard | Accepted | [system-monitor](../plans/system-monitor.md) |
| [0003](0003-classifier-agent-rust-loop-buildanagent-evals.md) | The classifier loop lives in Rust; buildanagent is used for evals | Accepted | [system-monitor](../plans/system-monitor.md) |
| [0004](0004-tauri-menu-bar-shell.md) | A Tauri menu-bar shell runs the existing server and page | Proposed | [storage-and-guide](../plans/storage-and-guide.md) |
