# data-files-readable-by-other-users

An existing `~/.monitor-lizard` folder was mode 755, and its history database and baseline were mode 644. Other local users could read the saved sudo and su launches. Only folders and files made by newer code were owner-only.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
