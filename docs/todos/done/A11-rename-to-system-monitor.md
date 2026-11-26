# A11: Rename the crate, binary and data folder to system-monitor

| Field | Value |
|---|---|
| ID | `A11` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `chore/a11-rename-to-system-monitor` |
| PR title | `chore(rename): rename ownwatch to system-monitor with a data migration` |
| Size | M (200-400) |
| PR | [#14](https://github.com/laudebugs/sys-monitor/pull/14) |

## Goal

Make the crate, binary, UI text, docs and data folder match the repository name, without losing the approved-apps baseline or history.

## Context

- The repository was renamed to `system-monitor` on 2026-11-26; the crate, binary and `~/.ownwatch/` still use `ownwatch`.
- Plan decision D5; open question 5 in the plan: confirm the final name with the owner first.

## Scope

**In scope:**

- Crate and binary name; header `X-Ownwatch-Token`; UI title and brand; README and docs text.
- One-time migration: if `~/.system-monitor` does not exist and `~/.ownwatch` does, copy it.

**Out of scope** (name the todo that covers it):

- Renaming the local checkout folder (a manual step).

## Target files

| File | Change |
|---|---|
| `Cargo.toml, Cargo.lock` | `name` |
| `src/baseline.rs, src/history.rs` | data directory constant and migration |
| `src/server.rs, web/index.html` | token header and strings |
| `README.md, AGENTS.md, docs/` | text |
| `docs/decisions/0002-rust-core-polling-collectors-local-web-ui.md` | add a dated `Amended` bullet (do not rewrite) |

## Steps

- [ ] Confirm the target name with the owner.
- [ ] Implement one `data_dir()` function used by both stores; add the migration with a test using a temp HOME.
- [ ] Rename everything else mechanically; grep for the old name.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Both directories exist | Use the new one; leave the old one untouched |
| Migration copy fails (permissions) | Warn once on stderr, start with empty state, do not delete the old directory |
| Fresh machine | Create only the new directory |

## Acceptance criteria

- `grep -ri ownwatch` finds only the ADR amendment and migration code.
- Existing baseline and history are present after first run.

## Tests

Rust test for `data_dir` migration with a temp HOME (3 cases above).

## Verification

```sh
make check
make test
grep -rIl ownwatch . --exclude-dir=target --exclude-dir=.git
```

## Docs to update

- All docs that mention the old name
- ADR 0002 amendment

## Risks and open questions

- Do this last: it touches every file and will conflict with every open branch.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** the GitHub repo `laudebugs/system-monitor` renamed to `laudebugs/sys-monitor` (GitHub redirects the old URL). `Cargo.toml` and `Cargo.lock` -> crate `sys-monitor`. New `src/paths.rs`: one `data_dir()` used by `baseline.rs` and `history.rs`, which copies `~/.ownwatch/` to `~/.sys-monitor/` once, on the first run that finds no new folder, and never touches the old folder. The write-guard header is now `X-SysMonitor-Token`. The dashboard's brand, page title, and `localStorage` keys (`sysmon-theme`, `sysmon-muted`) are renamed; the sidebar mark is logo concept 2 ("Circuit trace") from the owner's approved design, as an inline SVG matching the accent color in both themes. Prose now uses IBM Plex Sans (`--font-sans`) and data (`.num`, `.code`) keeps IBM Plex Mono (`--font-mono`); both are local-only font-family fallbacks, no remote font request was added (ADR 0002's no-network-calls decision holds). README, AGENTS.md, SECURITY.md, docs/agents/layout.md and the forward-looking `ready/` todos (A12, A13, A14, A16, B1, B2) that named the old binary or data path are updated; ADR 0002 and the plan got a dated amendment rather than a rewrite; historical Implementation Records (A1, A6) and this todo's own Context section keep the old name, since they describe what was true when written.
- **Tests:** 27 Rust tests pass (24 before; 3 new in `src/paths.rs`: a fresh machine creates only the new folder, migration copies data and leaves the old folder untouched, and when both exist the new one is kept as-is).
- **Real run on this Mac:** with `~/.sys-monitor/` removed, the first run copied `baseline.json` and `history.db` (188 KB) from `~/.ownwatch/`; the copies are byte-identical to the originals and `~/.ownwatch/baseline.json` is untouched afterward. `cargo build --release --locked` and `cargo test --locked` both succeeded after regenerating `Cargo.lock` for the renamed package (`cargo build --offline`, since `--locked` refuses to touch a stale lock file). The server answers on `http://127.0.0.1:4747`, the page title and nav brand read "Sys Monitor", a foreign `Host` header still gets 403.
- **Visual check:** a headless Chrome screenshot (dark theme, 1440x900) shows the new mark, brand, sans-serif prose and monospace numbers rendering correctly together. This is not the same as checking interactively in a real browser, which is still **pending**, including light mode and narrow widths.
- **Deviation from the sketch:** the repository itself was renamed (`system-monitor` -> `sys-monitor`), which the todo's title didn't call for; the owner asked for it directly ("let's actually rename the repo & app name to Sys Monitor") after this todo was written.
- **Caught and fixed mid-task:** a first pass at the UI edits was lost because one Python edit script aborted partway on a string-match miss (edits are applied in memory and only written at the end), which silently dropped the title, storage-key and theme-init renames; found by the real-run check (the page still said "ownwatch" in its `<title>`) and fixed before this record was written.
- **Open:** `/code-review` and `/writing-review` have not yet been run on this change.
