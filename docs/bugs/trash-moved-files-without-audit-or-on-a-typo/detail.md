# `POST /api/trash` moved files on a typo and wrote its audit row after the move

Resolution: [resolution.md](resolution.md)

## What we saw

A post-merge review of PR #60 (the S3a route) found, by reading the code:

- The doc comment said "a request that cannot be recorded moves nothing", but the code only checked that the database could be opened, moved every item, and wrote the row afterwards. A crash or a locked database mid-batch left items in the Trash with no record.
- Only the exact text `dry=1` was a dry run. `dry=true`, `dry=yes` or a typo moved real files.
- The request thread did the whole job: up to 50 ids, each re-measured for up to 10 s, so one request could hold the single-threaded loop (dashboard, `/api/kill`) for minutes.
- The checks ran once, then the move: a path swapped for a symlink in between was not caught.
- `HOME=/` or `/Users` was trusted; ownership was not checked, so a root-owned app failed with an OS error, not a reason.
- `confirm_yours=1` could be sent on the first call, so no step forced a preview.
- The audit table grew without bound, `/api/history` returned full paths without a token, and several `as i64` casts and `unwrap_or_default` calls hid errors.

## How to reproduce

Found by reading, then confirmed by tests written first against the new API: a `dry=true` query parsed as a real move; a database whose inserts abort let the old order move files and only then fail to record them; a mover that panics on the second of three items left no row for any of them.

## Measurements

Worst case before the fix: 50 ids x 10 s re-measure = 500 s of one thread, plus pacing sleeps of up to 250 ms. After: 45 s total budget, off the request loop.

## Cause

The guard was described, not enforced: the audit write was ordered after the effect, and the dry-run flag was an opt-in to safety (`dry=1`) rather than an opt-in to danger. Two sources of truth disagreed: the doc comment and the code.
