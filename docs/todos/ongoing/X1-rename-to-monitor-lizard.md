# X1: Rename the crate, binary and data folder to Monitor Lizard

| Field | Value |
|---|---|
| ID | `X1` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `chore/x1-rename-to-monitor-lizard` |
| PR title | `chore(rename): rename Sys Monitor to Monitor Lizard with a data migration` |
| Size | M (200-400) |
| PR | — |

## Goal

Make the crate, binary, UI text, docs and data folder match the new product name, Monitor Lizard, without losing the approved-apps baseline or history. Same shape as the earlier ownwatch -> Sys Monitor rename (A11): one more hop on the same migration chain.

## Context

- The owner asked to rename the app to "Monitor Lizard" and the GitHub repository to `monitor-lizard` (2026-11-26). The repository has already been renamed (`laudebugs/sys-monitor` -> `laudebugs/monitor-lizard`; GitHub redirects the old URL) and local remotes updated; this todo is the code-side follow-up, exactly mirroring A11's own sequencing (repo renamed first, crate/binary/UI/docs renamed in a dedicated PR after).
- The owner was asked whether the sidebar/favicon mark should change to something lizard-themed and chose to keep the existing circuit-trace mark: **no new design work in this todo**, only name/text/identifier changes.
- `src/paths.rs` already has one `data_dir()` migration chain (old `.ownwatch` -> `.sys-monitor`); this todo extends it one more hop, `.sys-monitor` -> `.monitor-lizard`, without touching or removing the existing `.ownwatch` migration step.
- This todo was authored and claimed directly into `ongoing/` in its own worktree rather than first published to `ready/` on `main`, since other todos (T1a and others) are actively in flight on `main` from a parallel session; this avoids an extra direct-to-`main` commit for a todo nobody else needed to pick from the queue.

## Scope

**In scope:**

- Crate and binary name (`sys-monitor` -> `monitor-lizard`).
- Data directory migration: `.sys-monitor` -> `.monitor-lizard`, chained after the existing `.ownwatch` -> `.sys-monitor` step (both migrations run in sequence on a machine that still has the oldest folder; the already-tested `.ownwatch` step is not modified, only extended).
- Write-guard token header (`X-SysMonitor-Token` -> a name matching the new product name).
- UI title, brand text, `localStorage` key prefixes (`sysmon-*`).
- README, AGENTS.md, SECURITY.md, docs/agents/layout.md, and every `ready/`+`ongoing/` todo that names the old binary, data path or token header.
- `docs/decisions/0002-rust-core-polling-collectors-local-web-ui.md`: a dated `Amended` bullet (never rewrite the original).

**Out of scope** (name the todo that covers it):

- A new logo/icon: owner chose to keep the existing circuit-trace mark.
- Renaming the local checkout folder(s): a manual step for the owner, same as A11.
- Rewriting any already-`done/` todo's historical Implementation Record: those describe what was true when written and are never rewritten (same rule A11 followed for A1/A6).

## Target files

| File | Change |
|---|---|
| `Cargo.toml`, `Cargo.lock` | `name` |
| `src/paths.rs` | a second migration hop, `.sys-monitor` -> `.monitor-lizard`, chained after the existing one |
| `src/server.rs`, `web/index.html` | token header and strings |
| `README.md`, `AGENTS.md`, `SECURITY.md`, `docs/` | text |
| `docs/decisions/0002-rust-core-polling-collectors-local-web-ui.md` | add a dated `Amended` bullet |

## Steps

- [ ] Extend `data_dir_in`'s migration chain with a test using a temp HOME (fresh machine; only `.sys-monitor` present; only `.ownwatch` present, chaining through both hops; both `.sys-monitor` and `.monitor-lizard` present).
- [ ] Rename everything else mechanically; grep for the old name.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Only `.sys-monitor` exists | Copied once to `.monitor-lizard`; `.sys-monitor` left in place |
| Only the oldest `.ownwatch` exists (never upgraded past A11) | Both hops run in sequence; ends with `.monitor-lizard` populated, both older folders left in place |
| `.monitor-lizard` already exists | Used as-is; no migration attempted from either older folder |
| Migration copy fails (permissions) | Warn once on stderr, start with empty state, do not delete the source directory |
| Fresh machine | Create only the new directory |

## Acceptance criteria

- `grep -ril "sys.monitor\|sysmon" .` (case-insensitive, excluding `target/` and `.git/`) finds only: the migration code and its comments (`src/paths.rs`), the two dated ADR 0002 amendments (A11's and this one), migration notes in README/SECURITY.md/layout.md and the living plan docs' resolved-decision text, this todo's own Context, external design-canvas titles quoted verbatim (S2a, S2b), and the historical record of anything written to describe a past state rather than the current one and never rewritten: already-`done/` todos' own prose and Implementation Records, ADR 0004 and its linked Tauri research spike (both are dated measurements taken under the old name), and the forensic bug report that quotes the page's literal old `<title>` text.
- Existing baseline and history are present after first run on a machine with only the old data folder.

## Tests

Rust tests for the extended `data_dir` migration chain (the table above: 5 cases).

## Verification

```sh
make check
make test
grep -rIl "sys-monitor\|sysmon\|Sys Monitor" . --exclude-dir=target --exclude-dir=.git
```

## Docs to update

- All docs that mention the old name
- ADR 0002 amendment

## Risks and open questions

- Do this one worktree at a time: it touches nearly every file and will conflict with any other open branch. Other todos (e.g. T1a) are in flight on `main` from a parallel session at the time this was written; this branch will need to rebase onto `main` again right before merging to pick up anything that landed meanwhile.

## Definition of done

- [ ] Claimed: first commit added this file directly to `ongoing/` (authored and claimed in the same step, per its own Context note, rather than published to `ready/` first)
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
