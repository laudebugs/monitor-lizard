# The CI test job stalls after `real_a_root_owned_process_is_refused_by_its_owner`

Resolution: [resolution.md](resolution.md)

## The job printed no results for 35 minutes

`make test` in the `code` job printed `real_a_root_owned_process_is_refused_by_its_owner ... ok` and then nothing. Runs on three pull requests were cancelled at the job's 35-minute limit. Other runs of the same code passed in under a minute.

## It does not reproduce on a fast disk

Run `cargo test --locked --lib` on a Mac and watch the last test to finish. It is `history::trash::tests::the_trash_audit_keeps_only_the_newest_rows`, and it passes. The stall was not reproduced. The cause below is inferred from local timing, and the sync time on a hosted runner was not measured (as of November 2026).

## Local timing points at one test

| Run | Time to finish |
|---|---|
| All library tests in parallel, local Mac | 14.8 s |
| That one test, local Mac | about 10 s |
| The same job on a hosted runner | no result for 35 minutes, then cancelled |

Run one at a time on the local Mac, three tests took longer than 1.5 s: two disk-image mount tests, which CI skips, and the audit test at 9.7 s.

## The test made 10,005 separate commits

The test called `begin_trash` 10,005 times to fill the audit past its 10,000-row cap. The baseline-changes audit test did the same with `record_baseline_change`, two statements per row. Each call is its own SQLite transaction in WAL mode, so each waits for a disk sync. At 1 ms per sync that is 10 s. At a few hundred milliseconds per sync it is over half an hour. The test was slow rather than stuck, and it was the last one still running, so the output stopped after the test that finished just before it.
