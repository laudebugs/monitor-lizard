# A11: Rename the crate, binary and data folder to system-monitor

| Field | Value |
|---|---|
| ID | `A11` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `chore/a11-rename-to-system-monitor` |
| PR title | `chore(rename): rename ownwatch to system-monitor with a data migration` |
| Size | M (200-400) |
| PR | — |

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
make check\nmake test\ngrep -rIl ownwatch . --exclude-dir=target --exclude-dir=.git
```

## Docs to update

- All docs that mention the old name
- ADR 0002 amendment

## Risks and open questions

- Do this last: it touches every file and will conflict with every open branch.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
