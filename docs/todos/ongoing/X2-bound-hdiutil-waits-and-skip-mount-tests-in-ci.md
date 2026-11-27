# X2: Bound the hdiutil waits and skip the mount tests in CI

| Field | Value |
|---|---|
| ID | `X2` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | None |
| Branch | `fix/ci-hdiutil-hang` |
| PR title | `fix(test): bound the hdiutil waits and skip the mount tests in CI` |
| Size | M (200-400) |
| PR | — |

## Goal

Stop `make test` in the GitHub Actions macOS `code` job from freezing the runner now and then, and remove one flaky test. The two `hdiutil` tests in `src/storage/trash.rs` are the suspected cause of the freeze, so every wait they make gets a hard limit, and CI skips them. The mount-crossing refusal is therefore verified locally only. The flake is `real_sleep_processes_freeze_resume_and_stop` in `src/requester_signal.rs`.

## Context

- A runner froze with no log uploaded, even though a watchdog should have fired. That points at the whole runner, not the test process. It happened on plain `main` too, so the history split (PR #73) did not cause it.
- Evidence for `hdiutil` (suspected, not proven): a `ps` snapshot from a passing run showed `hdiutil detach -force` still running after 9 s, with `diskimages-helper` and `hdiejectd` alive. The old `run_within` killed the child on timeout and then called `child.wait()` with no limit, and a process stuck in the kernel can ignore the kill.
- The flake failed once in CI with `signal_failed ... No such process (os error 3)`. Cause: on macOS a SIGTERM sent to a stopped process ends it at once (checked locally: after STOP then TERM the process was gone with no CONT). The follow-up CONT can then find the pid already reaped and fail with ESRCH, so a Stop that worked was reported as `signal_failed`. A 100-run loop on the old code did not reproduce it, so the race needs a loaded machine.
- [SECURITY.md](../../../SECURITY.md) and plan D15 name the mount checks that these tests prove.

## Scope

**In scope:**

- `run_within` polls for a few seconds after the kill, then abandons the child and returns an error that names the command and the elapsed time. No `wait()` follows a kill.
- Every test `hdiutil` call has a limit, and a Drop guard detaches with `-force`, then deletes the `.dmg`. The ignored walk test gets the same bounds.
- The two mount tests skip when `GITHUB_ACTIONS` or `CI` is set, print a loud notice, and run anyway with `ML_RUN_MOUNT_TESTS=1`.
- A Stop whose TERM already ended a frozen process is reported as signalled; the CONT that found nothing is recorded as unsent.
- The real-process test verifies each process is running before it signals, and every wait has a hard limit and a message that names the step.
- A step-level `timeout-minutes: 12` on `make test`.

**Out of scope:** replacing the hdiutil tests with a fake volume (they exist to exercise the real `st_dev` check); proving the freeze cause on a hosted runner.

## Target files

| File | Change |
|---|---|
| `src/storage/trash.rs` | Bounded `run_within`, `skip_mount_tests`, `Mounted` guard that also deletes the image |
| `src/storage/walk.rs` | The ignored mount test uses the bounded helper and the same guard rules |
| `src/requester_signal.rs` | Treat a follow-up signal that finds the process gone as success; deterministic real-process test; `Guard` no longer signals a reused pid |
| `.github/workflows/ci.yml` | `timeout-minutes: 12` on the `make test` step |
| `docs/agents/commands.md`, `docs/agents/layout.md`, `SECURITY.md` | Say the mount refusal is verified locally and how to run the tests |

## Steps

- [ ] Claim: add this file in `ongoing/`.
- [ ] Bound `run_within` and the guards.
- [ ] Skip the mount tests in CI; add `ML_RUN_MOUNT_TESTS=1`.
- [ ] Fix the Stop of a frozen process and the real-process test.
- [ ] Add the step timeout.
- [ ] Document.

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

## Acceptance criteria

- `cargo test` passes with and without `GITHUB_ACTIONS=true`.
- After the mount tests, `hdiutil info` lists no image this test made.
- `real_sleep_processes_freeze_resume_and_stop` passed 100 of 100 runs in a loop.

## Tests

`run_within_kills_a_command_that_overruns_and_names_it_in_the_error`; `a_stop_whose_term_already_ended_the_frozen_process_is_a_success_not_a_failed_continue`; `a_failed_first_signal_is_still_a_failure_for_a_frozen_stop`; the existing real-process tests.

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

- [ ] Claimed: first commit added this file in `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
