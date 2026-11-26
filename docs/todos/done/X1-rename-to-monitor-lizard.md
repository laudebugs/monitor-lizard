# X1: Rename the crate, binary and data folder to Monitor Lizard

| Field | Value |
|---|---|
| ID | `X1` |
| Status | `done` |
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
- Data directory migration to `.monitor-lizard`, extending the existing migration (previously just `.ownwatch` -> `.sys-monitor`) to try each older name in turn and migrate directly from whichever is found -- a machine still on the oldest name migrates straight across, with no intermediate `.sys-monitor` folder created for a name nothing reads anymore; every older folder found is left in place.
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

- [x] Extend `data_dir_in`'s migration chain with a test using a temp HOME (fresh machine; only `.sys-monitor` present; only `.ownwatch` present; both `.sys-monitor` and `.ownwatch` present; `.monitor-lizard` already present).
- [x] Rename everything else mechanically; grep for the old name.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Only `.sys-monitor` exists | Copied once to `.monitor-lizard`; `.sys-monitor` left in place |
| Only the oldest `.ownwatch` exists (never upgraded past A11) | Migrated directly to `.monitor-lizard` (no `.sys-monitor` is ever created as a throwaway middle step); `.ownwatch` left in place |
| Both `.sys-monitor` and `.ownwatch` exist | Migrated from `.sys-monitor` (the more recent one); `.ownwatch` is never read from and both older folders are left in place |
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

- [x] Claimed: first commit added this file directly to `ongoing/` (authored and claimed in the same step, per its own Context note, rather than published to `ready/` first)
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (mechanical rename; no new prose beyond this record)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `Cargo.toml`/`Cargo.lock` -> crate and binary `monitor-lizard`. `src/paths.rs`'s migration generalized from a single `OLD_NAME`/`NEW_NAME` pair to `OLDER_NAMES: &[&str]` (`.sys-monitor`, `.ownwatch`), tried newest-first; a machine still on the oldest name migrates straight to `.monitor-lizard`, never creating a throwaway `.sys-monitor` folder along the way, and every older folder found is left untouched. Write-guard header `X-SysMonitor-Token` -> `X-MonitorLizard-Token`. UI title, brand text, breadcrumb and `localStorage` key prefixes (`sysmon-*` -> `monitor-lizard-*`) in `web/index.html`, with the matching JS test in `tests/web/url-state.test.mjs` updated. README, AGENTS.md, SECURITY.md, `docs/agents/{layout,commands}.md`, every `ready/`/`ongoing/` todo naming the old binary/data path/token header, and the living plan docs' resolved-decision text all updated. `docs/decisions/0002` got a second dated `Amended` bullet alongside A11's, neither rewritten. The existing circuit-trace mark was kept unchanged, per the owner's choice.
- **Deliberately left alone** (each is a dated record of a past state, not a description of the app today): already-`done/` todos' own prose and Implementation Records; ADR 0004 and its linked Tauri research spike (measurements taken under the old name); a forensic bug report quoting the page's literal old `<title>` text; two `ready/` todos (S2a, S2b) that quote an external design canvas's actual title verbatim.
- **Tests:** 138 Rust tests (131 before +7 in `src/paths.rs`, two more than the todo's own 5-row table: besides fresh-machine, newest-only, oldest-only-direct, already-present, and the permission-failure case, added a dedicated test for both older folders present at once -- confirms the newer (`.sys-monitor`) is the one migrated from and the oldest (`.ownwatch`) is never read, not just never modified) plus 31 JS tests, all passing.
- **Real run:** built the release binary (genuinely named `monitor-lizard`), ran it against a scratch `HOME` with only `.ownwatch` present (migrated straight to `.monitor-lizard`, confirmed no `.sys-monitor` was created, `.ownwatch` untouched) and a separate one with only `.sys-monitor` present (migrated normally, untouched); started the real server and confirmed the page's `<title>` is "Monitor Lizard" and a real request with the renamed `X-MonitorLizard-Token` header succeeds.
- **The GitHub repository** was renamed directly at the owner's request (`laudebugs/sys-monitor` -> `laudebugs/monitor-lizard`, GitHub redirects the old URL) ahead of this PR, with local remotes updated, mirroring A11's own sequencing (repo renamed first, code following in a dedicated PR).
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Both independently caught the same real gap: `README.md`'s and `docs/agents/layout.md`'s migration notes still said "migration from `~/.ownwatch/`" only, silently dropping the `.sys-monitor` hop that `SECURITY.md` and the ADR amendment already described correctly -- fixed to name both older names, as above. Both also flagged that the Edge cases table's "both hops run in sequence" wording didn't match the actual (better) implementation, which migrates directly from whichever older name is found rather than literally chaining two sequential copies through a folder nothing reads anymore -- the table and Scope/Steps text were reworded to describe the real behavior accurately, and the missing "both older folders present" case was added as its own row and test rather than left implied. No scope creep found by either review: the icon/SVG content is byte-identical, no `done/` Implementation Record was rewritten, no todo ID appears inside any `.rs` or `.html` source file.
- **Deviation:** the migration design (direct-from-whichever-is-found, rather than two literal sequential hops through an intermediate folder) is a sound simplification over the todo's original "both hops run in sequence" phrasing, corrected in the todo text itself rather than left as a silent mismatch between spec and implementation.
- **Open:** `src/guide.rs`'s help-topic machinery still uses `sm-*`-prefixed internal IDs inherited from before this rename (pre-existing, unrelated to this todo's scope, low priority).
- **CI:** pending on the pull request.
