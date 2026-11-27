# Resolution: the CI test job sits in `make test` until it is cancelled

Detail: [detail.md](detail.md)

## The process tests are now lock tests with time limits

Both process tests and their two child tests are gone. Nothing in the replacements starts a process, and every wait has a limit, so a broken lock fails an assertion with a message instead of hanging the run.

The baseline gained three tests:

| Test | What it proves | Longest wait |
|---|---|---|
| `a_held_baseline_lock_refuses_a_second_holder_until_it_is_dropped` | While one handle holds the lock file, a second handle is refused; after the drop it gets the lock | 5 s |
| `a_change_waits_for_a_baseline_lock_held_elsewhere_and_then_lands` | A change started while another handle holds the lock is still waiting after 300 ms; once the lock is dropped it finishes and the mark is on disk | 10 s |
| `the_baseline_lock_is_held_while_the_change_is_read_and_written` | Between reading the file and writing it, a second handle cannot take the lock | none |

Together these cover what the old baseline test did. The old test proved that marks from two processes were not lost. That holds when the lock excludes other holders and `change_at` holds it from the read to the write. The existing test `concurrent_marks_from_threads_all_land` still checks that marks from threads are not lost.

The migrations threads test, `four_threads_opening_at_once_all_succeed_apply_the_schema_once_and_make_one_backup_of_the_old_database`, took over the backup checks. The backup is at version 0, holds the seeded row with `hits` 7, and is mode 0600. It is the only `.bak` file, so no temporary backup is left. Each thread opens the lock file itself, so the threads exclude each other as the processes did.

## A released lock can stay held for about a millisecond

In a full test run, the lock exclusion test failed in 4 of 12 runs when it checked the released lock once. It passed in 60 of 60 runs on its own. A retry showed the lock came free about 1.3 ms after the drop. The likely cause is a process that another test started at that moment, holding an inherited copy of the descriptor until it ran its program. This cause was not proven.

Both lock exclusion tests, the new baseline one and the existing migrations one, now retry for up to 5 s. After the change, 15 full runs passed.

## Each new test fails when the lock it guards is broken

Each break below was made by hand, the tests were run, and the code was restored. Every failure ended in under a second with the message shown.

| Break | Tests that failed | Message |
|---|---|---|
| `lock_file` opens the file but does not lock it | all three baseline lock tests | "a second handle got the baseline lock while the first still held it", "the change finished (Ok(true)) while another holder had the baseline lock", "another process could take the baseline lock between the read and the write" |
| `change_at` skips the lock file | the waiting test and the held-lock test | the last two messages above |
| `change_at` drops the lock as soon as it takes it | the held-lock test | "another process could take the baseline lock between the read and the write" |
| `History::at` skips the open lock | the four-threads test, in 30 of 30 runs | "every opener succeeds: ... could not create the backup file ... File exists" |

## How it was checked

| Check | Result |
|---|---|
| The baseline and migrations tests, run 40 times in a loop | 40 of 40 passed, 33 s in all |
| `cargo test --locked --lib`, after the retry was added | 15 of 15 passed, about 5 s each |
| `make check` and `make test` | passed |
| The `code` job on a hosted runner | not yet run on this branch |

## The CI stall is not proven fixed

Removing tests that could wait forever takes away one way for `make test` to never finish. The stall was never reproduced, so it is not proven fixed. If it happens again, find the test that was still running when the output stopped.

Other tests still start processes. Each one waits on a short shell command, kills its child, or polls with a time limit. One test, `unblock_ends_the_request_loop_instead_of_blocking_forever`, joins a thread with no time limit, and says so in its comment.
