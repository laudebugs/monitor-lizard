# Resolution: the CI test job sits in `make test` until it is cancelled

Detail: [detail.md](detail.md)

## The fix: tests that start no process and wait at most 30 s

Both process tests and their two child tests are gone. The replacements start no process. Every wait in them has a limit, so a broken lock fails an assertion with a message instead of hanging the run.

The baseline gained three tests:

| Test | What it proves | Longest wait |
|---|---|---|
| `a_held_baseline_lock_refuses_a_second_holder_until_it_is_dropped` | While one handle holds the lock file, a second handle is refused; after the drop it gets the lock | 5 s |
| `a_change_waits_for_a_baseline_lock_held_elsewhere_and_reads_the_file_only_after_it_gets_the_lock` | A change started while another handle holds the lock is still waiting after 300 ms. A mark written meanwhile survives the change, so the change read the file only after it got the lock | 10 s |
| `the_baseline_lock_is_still_held_between_the_read_and_the_write` | When the change is recorded in the history, after the read and before the write, a second handle cannot take the lock | none |

Together these cover what the old baseline test did. The old test proved that marks from two processes were not lost. No mark is lost when the lock excludes other holders and `change_at` holds it from before the read until the write. The existing test `concurrent_marks_from_threads_all_land` still checks that marks from threads are not lost.

The migrations threads test, `four_threads_opening_at_once_all_succeed_apply_the_schema_once_and_make_one_backup_of_the_old_database`, took over the backup checks. The backup is at version 0, holds the seeded row with `hits` 7, and is mode 0600. It is the only `.bak` file, so no temporary backup is left. The test now waits at most 30 s for all four openers.

## A released lock can stay held for about a millisecond

As of November 2026, the lock exclusion test failed in 4 of 12 full test runs when it checked the released lock once. It passed in 60 of 60 runs on its own. A retry showed the lock came free about 1.3 ms after the drop. The likely cause is a process that another test started at that moment, holding an inherited copy of the descriptor until it ran its program. This cause was not proven.

Both lock exclusion tests, the new baseline one and the existing migrations one, now retry for up to 5 s through one shared helper. The program itself always waits for a lock, so the delay does not affect it.

## Each new test fails when the lock it guards is broken

Each break below was made by hand, the tests were run, and the code was restored. No run hung. The baseline runs finished in under a second of test time, each with the message shown.

| Break | Tests that failed | Message |
|---|---|---|
| `lock_file` opens the file but does not lock it | all three baseline lock tests | "a second handle got the baseline lock while the first still held it", "the change finished (Ok(true)) while another holder had the baseline lock", "another process could take the baseline lock between the read and the write" |
| `change_at` skips the lock file | the waiting test and the held-lock test | the last two messages above |
| `change_at` drops the lock as soon as it takes it | the held-lock test | "another process could take the baseline lock between the read and the write" |
| `change_at` reads the file before it takes the lock | the waiting test | "the change read the file before it had the lock, so it lost the mark written while it waited" |
| `History::at` skips the open lock | the four-threads test, in 30 of 30 runs | "every opener succeeds: ... could not create the backup file ... File exists" |

Without the open lock, the four-threads test fails because the threads share a process ID. Two of them build the same temporary backup name, and the second cannot create it. A thread that backed up an already-migrated file would also fail, at the backup's version check.

## How it was checked

All checks ran on a local Mac, as of November 2026.

| Check | Before | After |
|---|---|---|
| Tests that start a copy of the test binary | 2 | 0 |
| Waits with no limit in the tests this change touched | 5 (two polling loops, two `wait()` calls, one thread join) | 0 |
| The baseline and migrations tests, run 40 times in a loop | not run | 40 of 40 passed, 36 s in all |
| `cargo test --locked --lib`, after the retry was added | not run | 15 of 15 passed, about 5 s each |
| `make check` and `make test` | passed | passed |

## Left for later

- The `code` job on a hosted runner has not yet passed on this branch. The stall was never reproduced, so it is not proven fixed.
- If the waiting test ever times out, its writer thread stays blocked on the lock while holding the baseline's in-process lock. Later baseline tests in that run would then wait too. This can only follow an earlier failure. Changing it would mean changing the program's lock code, so it is left as it is.
- Other tests still wait with no limit, though none waits on a start file. `unblock_ends_the_request_loop_instead_of_blocking_forever` and `concurrent_marks_from_threads_all_land` join threads. The web tests' `fetch` helper joins its server thread and reads its reply with no timeout. Each test that starts a process waits on a short shell command, kills its child first, or polls with a time limit.

Removing tests that could wait forever takes away one way for `make test` to never finish. If the stall happens again, find the test that was still running when the output stopped.
