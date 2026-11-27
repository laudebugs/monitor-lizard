# Resolution: the CI test job stalls after the last requester test

Detail: [detail.md](detail.md)

## The fix

The test now inserts the first 10,000 requests with one SQL statement and sends only the last five through `begin_trash`. The cap and the pruning are still what the test checks: the table ends at 10,000 rows and the newest survives.

## How it was checked

| Check | Before | After |
|---|---|---|
| `the_trash_audit_keeps_only_the_newest_rows` on the local Mac | about 10 s | 0.06 s |
| Rows left after 10,005 requests | 10,000 | 10,000 |
| Newest row survives | yes | yes |

The test itself is the regression test: it still fails if the pruning stops working, and it no longer depends on disk speed.

## Left for later

Other tests that commit in a loop would have the same problem on a slow disk. None was found: the next slowest tests are the two mount tests that CI already skips.
