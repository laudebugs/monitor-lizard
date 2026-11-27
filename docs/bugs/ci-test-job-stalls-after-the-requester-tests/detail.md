# The CI test job stalls after the last requester test

Resolution: [resolution.md](resolution.md)

## What we saw

`make test` in the `code` job printed `real_a_root_owned_process_is_refused_by_its_owner ... ok` and then nothing. Runs on three pull requests were cancelled at the job's 35-minute limit. Other runs of the same code passed in under a minute.

## How to reproduce

Run `cargo test --locked --lib` on any Mac and watch the last test to finish. It is `history::trash::tests::the_trash_audit_keeps_only_the_newest_rows`. It does not stall on a fast local disk, so reproducing the 35 minutes needs a slow disk.

## Measurements

| Run | Time to finish |
|---|---|
| All library tests in parallel, local Mac | 14.8 s |
| That one test, local Mac | about 10 s |
| The same job on a hosted runner | over 34 minutes, then cancelled |

Run one at a time on the local Mac, only three tests took longer than 1.5 s: two disk-image mount tests (skipped in CI), and the audit test at 9.7 s.

## Cause

The test called `begin_trash` 10,005 times to fill the audit past its 10,000-row cap. Each call is its own SQLite transaction in WAL mode, so each one waits for a disk sync. At 1 ms per sync that is 10 s. At a few hundred milliseconds per sync, which a busy hosted runner can reach, it is over half an hour. The test was slow rather than stuck, and it was the last test still running, so the output stopped after the test that finished just before it.
