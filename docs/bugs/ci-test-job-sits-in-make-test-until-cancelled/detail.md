# The CI test job sits in `make test` until it is cancelled

Resolution: [resolution.md](resolution.md)

## One run passed in 14 s and another sat for over 11 minutes

Two runs of the `code` job on the same code behaved differently in `make test`. One finished it in 14 s. The other was still in it after 11 minutes and was cancelled at the job's 35-minute limit. An earlier stall had a cause that was fixed first: see [ci-test-job-stalls-after-the-requester-tests](../ci-test-job-stalls-after-the-requester-tests/detail.md). This stall came after that fix.

## It does not reproduce on a local Mac

Run `cargo test --locked --lib` on a Mac. Every run in this investigation passed in about 5 s, and none stalled (as of November 2026). The log of the stalled CI run does not name the test that was still running. So the cause below is a suspect, not a measured fact.

## Two tests could wait forever on another process

Two tests started two copies of the test binary and waited for them with no time limit:

| Test | What it checked |
|---|---|
| `baseline::tests::marks_from_two_real_processes_all_land` | 20 marks from two processes all reach `baseline.json` |
| `history::migrations::tests::two_real_processes_opening_at_once_make_one_backup_of_the_old_database` | Two processes opening an old database make one owner-only backup of it |

Both had the same shape. Each copy polled every millisecond for a start file, with no time limit. The parent test slept 300 ms, wrote the start file, then called `wait()` on each copy, also with no time limit.

That shape can wait forever in two ways. First, a copy that never finishes, for any reason, keeps the parent in `wait()` and the test never ends. Second, a parent that fails before it writes the start file leaves both copies polling forever. Whether either happened on the runner is unknown, because the log stops without naming a test.

## The process tests checked two things that need no second process

The tests needed a second process only to get a second lock owner. Both lock files use `flock`, and a `flock` belongs to the open file, not to the process. So two handles opened separately in one process exclude each other exactly as two processes would. The existing test `the_open_lock_excludes_other_holders_in_the_same_process` already relied on this.
