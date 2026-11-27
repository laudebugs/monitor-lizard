# Resolution: the CI test job stalls after `real_a_root_owned_process_is_refused_by_its_owner`

Detail: [detail.md](detail.md)

## The fix

Each audit test now inserts its first 10,000 rows with one SQL statement and sends only the last five through the real function (`begin_trash` or `record_baseline_change`). The cap and the pruning are still what the tests check: the table ends at 10,000 rows and the newest row survives.

## How it was checked

| Check | Before | After |
|---|---|---|
| `the_trash_audit_keeps_only_the_newest_rows`, local Mac | about 10 s | 0.06 s |
| Rows left after 10,005 requests | 10,000 | 10,000 |
| Newest row survives | yes | yes |
| The `code` job on a hosted runner | stalled | not yet run on this branch |

The two tests still fail if the pruning stops working. Nothing guards against the slowdown coming back, because the old version passed too, only slowly.

## One loop of about 200 commits remains

`trash.rs` has a test that commits about 200 times, to check the item cap. At a few hundred milliseconds per sync that takes about a minute, so it is left as it is. If the job stalls again, look first for tests that commit in a loop.
