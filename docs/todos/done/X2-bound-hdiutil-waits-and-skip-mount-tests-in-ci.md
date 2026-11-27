# X2: Bound the hdiutil waits and skip the mount tests in CI

| Field | Value |
|---|---|
| ID | `X2` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | None |
| Branch | `fix/ci-hdiutil-hang` |
| PR title | `fix(test): bound the hdiutil waits and skip the mount tests in CI` |
| Size | M (200-400) |
| PR | [#78](https://github.com/laudebugs/monitor-lizard/pull/78) |

## Goal

Stop `make test` in the GitHub Actions macOS `code` job from freezing the runner now and then, and remove one flaky test. The two `hdiutil` tests in `src/storage/trash.rs` are the suspected cause of the freeze, so every wait they make gets a hard limit, and CI skips them. The mount-crossing refusal is therefore verified locally only. The flake is `real_sleep_processes_freeze_resume_and_stop` in `src/requester_signal.rs`.

## Context

- A runner froze with no log uploaded, even though a watchdog should have fired. That points at the whole runner, not the test process. It happened on plain `main` too, so the history split (PR #73) did not cause it.
- Evidence for `hdiutil` (suspected, not proven): a `ps` snapshot from a passing run showed `hdiutil detach -force` still running after 9 s, with `diskimages-helper` and `hdiejectd` alive. The old `run_within` killed the child on timeout and then called `child.wait()` with no limit, and a process stuck in the kernel can ignore the kill.
- The flake failed once in CI with `signal_failed ... No such process (os error 3)`. Probable cause, reasoned and not proven: on macOS a SIGTERM sent to a stopped process ends it at once (a local STOP then TERM check showed the process gone with no CONT). The follow-up CONT can then find the pid reaped and fail with ESRCH, so a Stop that worked was reported as `signal_failed`. Nobody has observed what CONT returns on a process macOS already ended; that step is inferred. A 100-run loop on the old code did not reproduce the failure, so the race needs a loaded machine.
- [SECURITY.md](../../../SECURITY.md) and plan D15 name the mount checks that these tests prove.

## Scope

**In scope:**

- `run_within` polls for a few seconds after the kill, then abandons the child and returns an error that names the command and the elapsed time. No `wait()` follows a kill.
- Every test `hdiutil` call has a limit, and a Drop guard detaches with `-force`, then deletes the `.dmg`. The ignored walk test gets the same bounds.
- The two mount tests skip when `GITHUB_ACTIONS` or `CI` is set, print a loud notice, and run anyway with `ML_RUN_MOUNT_TESTS=1`.
- A Stop whose TERM already ended a frozen process is reported as signalled (`signals` is `["TERM"]`) and the CONT that found nothing is recorded as an `unsent CONT` row. This counts only when a re-probe shows the process gone: not found, a zombie, or a different known start time. A start time that cannot be read back is not proof.
- A pure `crosses(parent_dev, dev)` for the device comparison in the walker and the trash check, with a unit test that runs in CI.
- The real-process test verifies each process is running before it signals, and every wait has a hard limit and a message that names the step.
- A step-level `timeout-minutes: 12` on `make test`. It only helps while the runner stays alive: the step timeout is enforced by the runner agent on the VM, so a frozen VM falls back to GitHub's lost-heartbeat handling and the 30 minute job limit. The real protection is that the mount tests skip in CI.

**Out of scope:** replacing the hdiutil tests with a fake volume (they exist to exercise the real `st_dev` check); proving the freeze cause on a hosted runner.

## Target files

| File | Change |
|---|---|
| `src/storage/trash.rs` | Bounded `run_within`, `skip_mount_tests`, `Mounted` guard that also deletes the image |
| `src/storage/walk.rs` | The ignored mount test uses the bounded helper and removes its temp folder if `create` fails; `crosses` (new) |
| `src/requester_signal.rs` | Treat a follow-up signal that finds the process gone as success; deterministic real-process test; `Guard` no longer signals a reused pid |
| `.github/workflows/ci.yml` | `timeout-minutes: 12` on the `make test` step |
| `docs/agents/commands.md`, `docs/agents/layout.md`, `SECURITY.md` | Say the mount refusal is verified locally and how to run the tests |

## Steps

- [x] Claim: add this file in `ongoing/`.
- [x] Bound `run_within` and the guards.
- [x] Skip the mount tests in CI; add `ML_RUN_MOUNT_TESTS=1`.
- [x] Fix the Stop of a frozen process and the real-process test.
- [x] Add the step timeout.
- [x] Document.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A command that overruns its limit | Killed, then polled for 5 s, then abandoned; the error names the command and elapsed time |
| `hdiutil attach` times out after mounting | The guard still detaches with `-force` |
| `hdiutil detach` fails | Folders and image are left alone, never walked into |
| `GITHUB_ACTIONS` or `CI` set | Both mount tests print a notice and return |
| `ML_RUN_MOUNT_TESTS=1` with `CI` set | Both run |
| Stop of a frozen process whose TERM ends it | `ok`, signals `["TERM"]`, `unsent CONT` in the audit log |
| First signal fails | Still `signal_failed` |
| A guard's pid was reused | Not signalled |
| Follow-up CONT fails and the process is alive, or its start time reads back empty | Still `signal_failed` |
| Follow-up CONT fails and the process is gone, a zombie or the pid was reused | `ok`, `["TERM"]`, `unsent CONT` row |

## Acceptance criteria

- `cargo test` passes with and without `GITHUB_ACTIONS=true`.
- After the mount tests, `hdiutil info` lists no image this test made.
- `real_sleep_processes_freeze_resume_and_stop` passed 100 of 100 runs after the change. That is not evidence of the fix: the old code also passed 100 of 100 locally. Only the Fake-based unit tests demonstrate the new rule.

## Tests

`run_within_kills_a_command_that_overruns_and_names_it_in_the_error`; `a_stop_whose_term_already_ended_the_frozen_process_is_a_success_not_a_failed_continue` (gone, zombie, reused pid); `a_failed_follow_up_signal_stays_a_failure_when_the_process_may_still_be_alive` (alive, unreadable start); `ended_is_true_only_for_gone_zombie_or_a_different_known_start`; `after_a_term_ended_the_process_it_is_gone_and_its_pid_is_not_ours_to_continue`; `a_failed_first_signal_is_still_a_failure_for_a_frozen_stop`; `crosses_is_true_only_for_a_different_device`; the real-process test, which now checks the audit rows for the outcome it accepts; the existing real-process tests.

## Verification

```sh
make check
make test
RUST_TEST_THREADS=1 make test
RUST_TEST_THREADS=8 make test
GITHUB_ACTIONS=true cargo test mount
ML_RUN_MOUNT_TESTS=1 CI=true cargo test mount
hdiutil info
```

## Docs to update

`docs/agents/commands.md`, `docs/agents/layout.md`, `SECURITY.md`.

## Risks and open questions

- The freeze cause is not proven. If a runner still freezes with the mount tests skipped, look elsewhere (another test that spawns system tools).
- A hosted runner's step timeout may not fire if the whole machine freezes; it only helps where the job can still run its own watchdog.
- The mount-crossing refusal now has no CI coverage.

## Definition of done

- [x] Claimed: first commit added this file in `ongoing/`
- [x] Follows the applicable guidelines
- [x] Two independent reviews (standards and spec) ran on PR #78; their findings are fixed in the follow-up commit
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green locally (default, 1 and 8 threads); CI on the PR green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27. Built as described, with these points:

- `run_within` is bounded (5 s reap window, then the child is abandoned and lives until the test binary exits); the `Mounted` guards detach with `-force` and delete the image. The walker's ignored test removes its temp folder if `hdiutil create` fails.
- The two trash mount tests skip when `GITHUB_ACTIONS` or `CI` is set, unless `ML_RUN_MOUNT_TESTS=1`. The mount-crossing refusal is verified on a developer's machine only; the device comparison itself is now `walk::crosses`, unit-tested in CI. No behaviour change: the same `!=` comparison, moved into a function.
- Deviation from the sketch: the flake needed a product change in `src/requester_signal.rs` (`ended`/`gone`). Nobody has seen CONT fail on a process macOS had already ended; the mechanism is inferred from a local STOP then TERM check.
- Still open: CI on the PR has not run, so the step timeout and the skip are unobserved on a hosted runner; the freeze cause is unproven; the ESRCH race was never reproduced locally.

Result after merge, 2026-11-27: #78 merged as 60c5771; the next PR runs (#78's own and #73) passed `make test` in 1 to 3 minutes. The freeze was intermittent before (about 2 of 8 CI jobs), so a few green runs are not proof; its cause is still unproven. Watch the next ten PR runs; if a hang returns, the 12-minute step limit makes it fail fast.
