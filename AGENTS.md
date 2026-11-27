# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository. `CLAUDE.md` is a symlink to this file, so edit `AGENTS.md` only.

This file holds what every session needs. Detail lives in `docs/agents/`, one file per topic: read the one for your area before you start. Keep this file short; when it grows, move the detail out and leave a pointer.

## What this project is

Monitor Lizard (crate and binary `monitor-lizard`) is a local-first macOS security monitor for one person's Mac. It groups every process and network socket by app, flags what is not yours, and can stop your own processes after a confirmation. The repo holds a Rust collector and server (`src/`), a web dashboard (`web/`: one shell, one stylesheet, classic scripts; see `docs/agents/layout.md`), and the docs.

Decisions are recorded as ADRs (architecture decision records) in `docs/decisions/`. Do not assume a decision that is not written there.

## Read the detail doc for your area first

| Read | Before you work on |
|---|---|
| `docs/agents/layout.md` | Adding or moving files and folders |
| `docs/agents/commands.md` | Running checks, tests, or builds |
| `docs/agents/ci-and-release.md` | CI, releases, dependency updates |
| `docs/guidelines/git-workflow.md` | Branches, worktrees, commits, PRs, merging |
| `docs/guidelines/bug-triage.md` | Diagnosing and recording a bug |
| `docs/guidelines/maintainability.md` | Size budgets, duplication, dead code, baselines |
| `docs/guidelines/docs-writing.md` | Any prose a human will read |
| `docs/standards/` | Coding, writing, and other standards for this project |

Update the `docs/agents/` file a change affects **in the same PR**.

## Repository layout

```
AGENTS.md  CLAUDE.md -> AGENTS.md   agent rules (this file)
Makefile                            single task entry point; CI runs the same targets
.agents/skills/                     skills (source of truth); .claude/skills symlinks here
docs/                               everything written down; every folder has a README index
  agents/ decisions/ plans/ todos/{ready,ongoing,done}/ bugs/
  research/ guidelines/ standards/ guides/ reference/
scripts/                            repo tooling (todo board, doc-index check, worktree helpers)
.github/                            workflows, dependabot, PR and issue templates
```

Cargo.toml  Cargo.lock              Rust crate `monitor-lizard` (ADR 0002)
src/                                collectors, grouping, rules, history, actions, local server
web/                                the dashboard: index.html shell, app.css, js files (vanilla JS, no build step, no remote assets)

## Twelve working agreements

1. **Work in a worktree, never on `main`.** `scripts/worktree-new.sh <type>/<desc>` creates one. Never commit on `main`; never stash (stashes are shared across worktrees; commit `wip:` instead). Remove the worktree after merge. Detail: `docs/guidelines/git-workflow.md`.
2. **Work from the todo board.** One todo is one branch and one PR. Pick the next todo in `docs/todos/ready/` whose `Depends on` are all in `done/`. Claim it in your first commit by moving it to `ongoing/`; move it to `done/` in your last work commit. The open PR is the lock: check `gh pr list` before starting.
3. **Plan before building.** Non-trivial work gets a plan in `docs/plans/` (copy `TEMPLATE.md`), then todos that link to it. Decisions that are hard to reverse get an ADR in `docs/decisions/`.
4. **Pull requests only.** Branch `<type>/<short-description>`; Conventional Commits (`type(scope): summary`); PR title is the squash commit. Squash-merge when CI is green and every review finding is fixed or explained.
5. **Review every PR before merging.** Run `/code-review high <PR#>` without `--comment`. Fix findings in new commits whose messages list each finding and its fix. If more than 6 findings were confirmed, run a second, lower-effort review. Prose changes also get `/writing-review`.
6. **Never patch code only to make a test pass.** Find out why it fails. Fix the product, or fix or remove a nonsensical test and say why in the commit message.
7. **Prove it works the way a user meets it.** UI changes are checked in a real browser; API-level tests do not prove a UI works. Record what you actually tested, and mark checks you could not run as pending.
8. **Document as you go.** Research, plans, decisions, bugs, and guides each have a folder. Add your file to that folder's README index in the same PR. Cite sources; label opinions.
9. **Comments say why, never which todo or review caused the code.** No todo IDs in source files; they belong in commits and plans.
10. **Never commit secrets.** Commit `.env.example`, never `.env`. Do not add a dependency without checking its licence.
11. **Use the project's skills** (`.agents/skills/`) and tell subagents to load them. Where a skill and a guideline disagree, the guideline wins; if the skill is better, change the guideline in its own PR.
12. **Name your session after its work**: `<AccountName>::<ProjectName>::<SessionName> - [<todo>:#<PR>]` (PascalCase). See the `session-name` skill.

## Environment quirks, dated, with the reason

- **2026-11-26: in zsh `log` is a shell builtin.** It ignores unified-log arguments and returns nothing, so sudo and authd investigations looked empty. Always call `/usr/bin/log`.
- **2026-11-26: the code is macOS only.** `ps`, `lsof`, `codesign`, `plutil` and `libproc` are used directly, so `cargo test` and `make check-code` need a Mac. CI's `code` job runs on `macos-latest`.
- **2026-11-26: the browser extension was not connected**, so UI changes could only be smoke-tested in a simulated DOM (logic, not layout). Under rule 7, record layout checks as pending until someone looks in a real browser.
- **2026-11-26: JavaScript tooling stays on Node.** The owner tried switching to Bun and found the buildanagent packages do not work under Bun yet. The two doc scripts themselves ran identically under Bun and Node, so the reason is the dev-time evals (todo A16), not the scripts. Revisit when buildanagent supports Bun.
- **2026-11-26: the existing Rust is not rustfmt-clean.** `make fmt` exists but is not part of `make check`; run it only in a PR that does nothing else, or the diff buries real changes.

## Run `make check` before every commit

`make help` lists everything. The ones you will use:

```
make check     # docs checks + your stack's gates: exactly what CI runs
make todos     # print the todo board (derived; never committed)
make doctor    # are the required tools installed?
make ci        # the whole fast lane locally
```

Detail: `docs/agents/commands.md`.
