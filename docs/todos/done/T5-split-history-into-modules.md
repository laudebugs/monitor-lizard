# T5: Split history.rs into modules

| Field | Value |
|---|---|
| ID | `T5` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), the file-size budget in [maintainability](../../guidelines/maintainability.md) |
| Depends on | None |
| ADRs | None |
| Branch | `refactor/t3-split-history` |
| PR title | `refactor(history): split history.rs into modules` |
| Size | L (a move; see Risks) |
| PR | [#73](https://github.com/laudebugs/monitor-lizard/pull/73) |

## Goal

Split `src/history.rs` (1,738 lines) into a `src/history/` folder where no file passes the 800-line budget, as a pure move: no behaviour change, no renamed or removed public item, and every caller keeps its `history::` path.

## Context

- Every PR that adds a table to the history database edits this file, so one 1,700-line file makes those PRs collide and hard to review.
- The module holds one `History` connection type, the open, lock and migration plumbing, and the code of eight tables (seen hosts, startup items, sudo launches, actions, baseline changes, the Trash audit, daily capacity, downsampled metrics).
- The existing code is not rustfmt-clean, so the move must not reformat anything.

## Scope

**In scope:**

- `src/history/mod.rs` keeps `History`, `open`/`at`, the data path and `open_or_report`, and re-exports every public item at its old path.
- One submodule per table, plus `migrations.rs`; each table's tests move with its code.
- `docs/agents/layout.md` names the modules and says where a new table goes.

**Out of scope** (name the todo that covers it):

- Any logic, schema or signature change; none planned.
- Running `make fmt`; a separate PR if the owner wants it.

## Target files

| File | Change |
|---|---|
| `src/history.rs` | removed; becomes `src/history/mod.rs` |
| `src/history/*.rs` | (new) one module per table, `migrations.rs`, and a test helper module |
| `docs/agents/layout.md` | module map |

## Steps

- [x] Claim: add this file to `ongoing/` (the branch was later rebuilt as one commit that adds it to `done/`; see the Implementation Record).
- [x] Move code by line range into the submodules; widen only the visibility of items that now cross a module boundary, to `pub(super)`.
- [x] Prove the move: public items and signatures, test names, item bodies.
- [x] `make check`, `make test`, `make check-docs`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A caller names `history::History`, `history::start_capacity_writer` and the other public items | Compiles unchanged |
| The two-process migration test re-runs itself by test name | The name in its `--exact` argument matches the moved test |

## Acceptance criteria

- No file in `src/history/` is over 800 lines.
- The set of `pub` items and their signatures is identical before and after.
- The leaf names of the Rust tests are identical before and after.
- Every moved item's text is identical apart from indentation and `pub(super)` on items that cross a module boundary.

## Tests

The existing tests, moved with their code. No test added or removed.

## Verification

```sh
make check
make test
```

The tests open databases, so run them with `HOME` and `TMPDIR` pointing at scratch directories.

## Docs to update

`docs/agents/layout.md`.

## Risks and open questions

- The diff is large but mechanical; review it with `git diff --color-moved`.
- A test's full name now includes its module (`history::migrations::tests::...`), so anything that names a history test by its full path must follow it.

## Definition of done

- [x] Claimed: the first commit added this file to `ongoing/` (the rebuilt branch carries it in `done/` only)
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages (an independent purity review found no behavioural change; see the commit message)
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

Built as designed: `src/history/` holds `mod.rs` and eight table modules plus `migrations.rs` and `test_util.rs`, none over 450 lines. Proof of purity (public items, test names, item bodies) is in PR #73. An independent review of the move found no behavioural change, and `/writing-review` was applied to this todo and the layout entry.

### Review and rebuild

- **Reviews.** Three independent reviews found no behavioural change: a purity review (tokenised diff of the old file against the new files, SQL and `MIGRATIONS` in order, test names equal, the re-exec test name checked by mutation, visibility audit), and then the `/code-review` skill on PR #73 with separate standards and spec reviewers. Their only points were two minor judgement calls on the standards side (a very long `layout.md` table cell, and four helpers widened to `pub(super)` so `History::at` can call them) and stale wording that this commit fixes.
- **Rebuilt branch.** The first three commits were unsigned, and branch protection requires signatures, so GitHub blocked the merge. The branch was rebuilt as one signed commit on the newest main. This todo was renamed from T3 to T5 because another todo took T3 first; the branch name keeps `t3`. An empty signed commit re-triggered CI.
- **CI hang.** On a base that included #75 and not yet #76 and #77, the `code` job's `make test` hung past 25 minutes on two runs. The same tree on main `76872d1` passed. The local suite passed at 1, 2 and default thread counts (485 Rust tests, 4 ignored). The cause of the hang is not established by this PR.
- **Test count.** 489 test names, the same before and after the move (485 run, 4 ignored).
