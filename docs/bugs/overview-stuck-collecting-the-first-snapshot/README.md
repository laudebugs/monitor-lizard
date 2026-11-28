# overview-stuck-collecting-the-first-snapshot

On first launch the dashboard stuck on "Collecting the first snapshot..." for up to 10 seconds, long enough to look hung; a reload fixed it instantly. The client polled a `null` `/api/snapshot` reply (the server's first collect+codesign pass still running) on the same 10 s cadence as any other update, instead of retrying soon.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
