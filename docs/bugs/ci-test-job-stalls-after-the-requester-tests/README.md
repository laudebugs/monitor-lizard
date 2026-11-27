# ci-test-job-stalls-after-the-requester-tests

The CI `code` job sometimes stopped printing test results for over half an hour and was cancelled at its 35-minute limit. One test wrote 10,005 separate database commits. It now seeds the rows in one statement.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
