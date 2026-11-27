# ci-test-job-stalls-after-the-requester-tests

The CI `code` job sometimes stopped printing test results and was cancelled at its 35-minute limit. Two database tests each made 10,005 separate commits. They now seed the rows in one statement.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
