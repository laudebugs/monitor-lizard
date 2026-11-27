# ci-test-job-sits-in-make-test-until-cancelled

The CI `code` job sometimes stayed in `make test` until it was cancelled, after another run on the same code passed in 14 s. The cause is not proven. Two tests that started copies of the test binary and waited with no time limit were replaced by tests that start no process and wait at most 10 s.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the suspected cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
