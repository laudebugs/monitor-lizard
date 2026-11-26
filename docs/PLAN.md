# ownwatch plan

Goal: continuously answer "is everything running and talking on this Mac mine?"

## Prior art (researched 2026-11; unverified items marked)
| Tool | Collection | Licence | Gap vs our goal |
|---|---|---|---|
| Little Snitch 6 | Network Extension + Endpoint Security | proprietary | Firewall; no "not mine" verdict, no persistence view |
| LuLu (Objective-See) | Network Extension (unconfirmed) | GPL-3.0 | Alerts/rules only; no inventory |
| KnockKnock, BlockBlock, TaskExplorer, Netiquette, OverSight, ReiKey | persistence scan, sockets, signing | GPL-3.0 | Best coverage, but split across apps; no history or baseline |
| Santa (northpolesec) | Endpoint Security exec logging | Apache-2.0 | Exec only, no network |
| osquery | SQL tables; es_process_events needs FDA | OSS | No UI or baselining |
| Mac Monitor (Red Canary) | Endpoint Security, ~41 event types | OSS | Event firehose, no verdicts, needs entitlement |
| Aftermath (Jamf) | one-shot forensic collection | MIT | Not continuous |
Unverified: Lynis, Fleet/Kolide, Vallum, Radio Silence, Murus, commercial EDRs.

**Our niche:** a continuous, grouped inventory with a baseline ("mine") and a first-seen diff, which none of the above combine.

## Apple API reality
- Endpoint Security needs the restricted `com.apple.developer.endpoint-security.client` entitlement. Reports of 5-13 month waits, per-team applications, and denials for distribution. Dev testing means a VM with SIP/AMFI off (2019 guidance; recheck on current macOS).
- Network Extension content filter needs a paid account (not verified) and only matters if we want blocking.
- NetworkStatistics.framework (used by nettop) is private. Do not depend on it.
- Free-tier path: polling via libproc / lsof / codesign / plist scans / unified log.

## Decisions
- **Rust core**, polling-first. Crates: `libproc` (done: real image path via proc_pidpath), `rusqlite` (history), `endpoint-sec` (phase 6, if entitlement granted). Shell out to `codesign`/`spctl` for signatures (apple-codesign looks stale).
- **UI**: local web dashboard now (`web/index.html`, served on 127.0.0.1). Later a Tauri v2 menu-bar shell showing the "unknown" count that opens the same dashboard.

## Phases
1. **Inventory (done)**: ps + proc_pidpath + lsof + codesign, grouping, rules, CLI, dashboard with approve.
2. **History + actions (done)**: SQLite first-seen remote host per app, flagged for approved apps (60s learning window, 1h flag window); background recorder every 10s.
3. **Baselines**: first-seen remote host per app; learning period; Trust / Ignore / Investigate inbox.
4. **Enrichment**: reverse DNS, ASN/owner.
5. **Persistence**: LaunchAgents/Daemons, login items, cron, profiles; diff vs baseline.
6. **Privilege events (done, v1)**: authd log parsed into admin requests; plus a 20 ms process-table watcher that records every sudo/su launch with args and parent chain (1% CPU). Limit: sub-20 ms invocations can be missed or lose their parent; Endpoint Security would close that gap.
7. **Real-time (optional)**: Endpoint Security helper. Apply for the entitlement early.
8. **Menu-bar app** (Tauri v2) and notifications for high findings only.

## Risks
1. ES entitlement may never be granted to an individual.
2. Polling misses short-lived processes and connections; other users' sockets need root.
3. No public non-root per-process packet attribution.
4. "Not mine" needs a trust model (signature, path, install source) to avoid false positives.
5. Unverified: SIP-off ES workaround on current macOS; NE terms for free accounts.

## Principles
Read-only, local-only (127.0.0.1, Host and Origin checks), no telemetry, every finding explainable in one line.

## Actions (done)
Stop/Quit send SIGTERM or SIGKILL via `/api/kill`. Guards: loopback Host and Origin checks, a random per-run token in `X-Ownwatch-Token` on every write, own-user processes only, never launchd or ownwatch itself, max 200 pids, and an audit row in `~/.ownwatch/history.db` (`actions` table). The UI hides Stop for root processes.

## Local classifier (research, 2026-11-26; unverified items marked)
**Principle:** rules decide the label, the model only explains and ranks suggested actions. It can escalate toward caution, never downgrade a rule-based "suspicious", and never acts on its own. Every action still goes through the confirm dialog.

**Default:** Qwen3-4B Q4_K_M (Apache-2.0, 2.5 GB) run as a `llama-server` subprocess (crash isolation, unloads when killed) with a flat JSON schema / GBNF grammar. HF: Qwen/Qwen3-4B-GGUF.
**Fallback (8 GB Macs):** Qwen3-1.7B (Apache-2.0, Q8 1.83 GB). Alternative: Phi-4-mini (MIT; card warns it hallucinates names/URLs). Avoid gated repos (Gemma) as a default.
**Feedback memory:** bge-small-en-v1.5 (MIT, 33M params) embeddings for similarity to apps you marked expected/suspicious. Not a standalone classifier. DeBERTa zero-shot gives a label only, no rationale.
**No security-specific small model worth shipping was found.** Accuracy on process metadata is unproven: build a labelled test set from your own machine before trusting it.

**Safety:** pin the HF commit SHA, verify SHA-256 after download and at load, manifest ships in the binary. Accept only GGUF/safetensors/ONNX, never pickle. Run inference in a subprocess with no network. Cap disk at ~3 GB with a delete button. Hub contact only for the explicit download; support offline pre-seeding.
**Prompt injection:** process names and command lines are attacker-controlled. Truncate to 256 chars, strip control characters, put them in quoted data fields only (never the system prompt), constrain output to an enum schema. Show rule evidence next to model text, label it "AI-generated, unverified".

**Input features:** exe_path_class, signed, team_id, notarized, parent, listening_ports, remote_host_count, first_seen_age_h, persistence, rule_label, rules_fired, user_baseline.
**Output (enforced):** `{label: benign|unknown|suspicious, confidence: low|medium|high, actions: [approve|ignore|investigate|quit|force_quit|block_network], rationale <= 240 chars}`; code overrides `label` with `rule_label` when the rule is stricter.

**UI:** label chip + confidence band (not a percentage) in the Apps table, sortable by uncertainty. Attention cards: rule label, one AI rationale line, at most one suggested action. Drawer: evidence first, then rules fired, then the model's text; raw name/command shown in a code box. If rules and model disagree or confidence is low: "Unsure, investigate", no quit suggestion. Confirm dialog defaults focus to Cancel; force quit and block network need a second step. "Mark as expected / suspicious" writes a local baseline keyed by team ID and path, with export and reset. First-run card shows model, size, licence, commit and SHA-256, a progress bar, and "Skip, rules only" (fully functional).

**Phases:** (a) rule-only labels + label column in UI; (b) `ownwatch model install` with pinned manifest and checksum; (c) llama-server integration + rationale; (d) feedback baseline + bge-small similarity.
**Risks:** prompt injection; confident wording giving false safety; unproven accuracy; download supply chain; memory pressure on 8 GB Macs; llama-cpp-2 is pre-1.0 (another reason to prefer the subprocess).
**Unverified:** RAM/latency on M-series, GGUF sizes for Phi-4-mini/Gemma/DeBERTa, GGUF parser CVEs, Ollama/LM Studio/MLX, mistralrs JSON-schema support, the Hub's SHA-256 field name.
