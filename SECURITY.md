# Security

Report vulnerabilities through GitHub's private vulnerability reporting (Security tab, "Report a vulnerability"). Do not open a public issue.

You can expect an acknowledgement within a few days. Say in the report what is affected and how to reproduce it.

## In scope

system-monitor can stop processes on your Mac, so these matter most:

- A way for a web page or another local process to stop a process, approve an app, or read data without the per-run token (CSRF, DNS rebinding, token leakage).
- A way to make `POST /api/trash` move anything the latest scan did not list, a symbolic link, or a path outside your home folder and `/Applications`, or to skip the second confirmation for an item labelled yours.
- A way to make it signal processes it refuses: root-owned, launchd, itself, or another user's.
- A way to make `POST /api/requester/signal` signal a process it refuses (root-owned, another user's, launchd, Monitor Lizard or its parents and children), a pid that is no longer the process that was captured, or anything without a preview token and an audit row.
- The server listening on anything other than 127.0.0.1.
- Injection through process names, command lines, or other attacker-controlled strings shown in the dashboard.

## Design guarantees

Loopback only, Host and Origin checks on every request, a random per-run token on every write, own-user processes only, an audit log of every action, no telemetry. Tests need no credentials and make no network calls.

`POST /api/kill` requires a `start=<unix-seconds>` for every pid in `pids=`, read moments earlier from `GET /api/processes`. Before signalling, the server re-reads that pid's start time fresh and refuses if it no longer matches, so a pid recycled by a new, unrelated process between the read and the click is never signalled. A request missing `start`, or with a different count of `start` values than `pids`, is refused outright rather than falling back to the old pid-only check.

## What `/api/requester/signal` does

The Privilege page can freeze (SIGSTOP), resume (SIGCONT), stop (SIGTERM) or force stop (SIGKILL) the processes that led to a `sudo` or `su`: one process, or a chain, meaning the chosen hop and every hop closer to the process that ran `sudo`. Side branches (other children of those processes) are listed but never signalled. The route needs the Origin check and the per-run token. Each process is named with the pid, start time and executable path the collector captured, and at most 12 are named per request. Only `dry=0` signals: no `dry` or `dry=1` is a preview, and any other value is rejected. A preview returns a confirmation that is good for ten minutes and one use, in memory only, and tied to the action, the scope, the danger flag and those exact items (each field is length-prefixed, so an executable path cannot imitate another item). One signalling request runs at a time (`busy`).

**What the server checks.** Every pid is read fresh three times: in the preview, before the audit rows are written, and immediately before its own signal. A pid is signalled only if all of these hold.

- It still exists and has the captured start time and executable (otherwise `pid_reused`). The start time has whole-second resolution, because that is what the collector recorded.
- It belongs to your user (otherwise `root_owned` or `not_yours`), read from the same `proc_pidinfo` call as its state, parent and start time.
- It is not pid 1 (`launchd`), and not Monitor Lizard or one of its parents or descendants (`monitor_lizard`). If the table of that tree cannot be built, every signal is refused (`unverifiable`).
- The action fits its state: a second freeze is `already_frozen`, resuming a running process is `not_frozen`.
- SIGCONT goes only to a process this app froze. The audit log must hold a STOP row for that pid, newer than the process start, with no later CONT or KILL. A process stopped by job control (^Z) or a debugger is `not_frozen_by_us`: Resume and Stop (which needs a CONT) are refused. Force stop still works, because SIGKILL needs no CONT.
- For `scope=chain`, each item's parent is the next item (otherwise `not_a_chain`; `chain_exited` when a process below it has exited), checked again before each signal. Stop goes child first, so by then a child this request already signalled may be gone, and that is accepted.
- A hop that looks like your own session needs `confirm_danger=1` on both the preview and the real request, or it is refused (`needs_danger_confirmation`); the flag is part of the confirmation's binding. Resume never needs it. This is a deliberate second step, not an authorisation boundary: anything that holds the per-run token can send the flag. The server reads four things. First, the file name of the executable and of the first word of the command line: a shell, `login`, `su`, `ssh`, a multiplexer such as `tmux`, `claude`, or a launcher such as `npx`, `tsx` or `electron`. Second, a terminal, editor or IDE app bundle on either path. Third, any word or folder named `claude`, `claude-code` or `claude.*`, because a native Claude Code install runs from `.../claude/versions/<version>`. Fourth, an interpreter whose command line is missing. An executable that cannot be read counts as your own session. The lists are in `src/requester_signal.rs`, and the page's copy is tested against the same fixture.

**What only the page does.** The warning text, the caution level for scripts and for processes with children, and the order of the two confirmation steps are the page's. A client that calls the route directly gets the same server checks, but a script or a process with children needs no extra flag.

Freeze and resume go parent first, stop and force stop child first. Stop on a frozen process also plans SIGCONT, in case the SIGTERM waits for it. macOS may end a stopped process on the SIGTERM itself; then the SIGCONT finds nothing, the response lists only `TERM`, and the audit log gets an `unsent CONT` row. That still counts as a successful stop, but only when the process is verifiably gone (not found, a zombie, or the pid now has another start time); any other failure of the follow-up is `signal_failed`. One audit row per signal that will be sent (STOP, CONT, TERM or KILL in the `actions` table, with the planned pids and anything refused) is written before the first signal; if one cannot be written nothing is signalled. A planned row does not prove that a signal went. Every planned signal that was refused at the last moment or failed therefore gets a compensating row (`unsent STOP`, `unsent CONT` and so on, with the pids it did not reach). The check that decides whether this app froze a process reads only signals that really went. If a compensating row cannot be written, the reply says so, and the log may then overstate what was sent.

`GET /api/requester/status` lists, for the processes it is asked about, whether each is running, frozen (and whether this app froze it), exited or replaced, and the command lines of the children of a process that verified in full. It needs the per-run token, because command lines can hold secrets. It lists no children for pid 1, a process of another user, an unverifiable item or a refused one. A request runs `ps` at most twice, and only after the token is checked: once for the table of Monitor Lizard's own tree, once for the command lines and children of all processes (the danger check reads command lines from that second table, so there is no `ps` per process). It also reports, for each process after the first, whether the chain up to it still holds together, so the page can switch the chain buttons off the way a preview would.

Limits: a process swapped for another in the instant between the last check and the signal is a race no check can close; the checks shrink it to the time between two reads, they do not remove it. The start time compares whole seconds. A freeze row in the log proves this app asked for the freeze, not that the process is still frozen by it.

## What `/api/storage` exposes

`POST /api/trash` is the one way the app removes files, and it only moves them to your Trash. Finder's "Put Back" may not be offered for those items; History keeps the original path of each one, so you can move it back by hand. The route needs the Origin check and the per-run token, and it takes ids from the latest scan, never paths. Only `dry=0` moves anything: no `dry` or `dry=1` is a preview, and any other value is rejected. A preview returns a confirmation that is good for ten minutes and one use, kept in memory only, and tied to those exact items, their labels and that scan; a real move must send it back. The server checks each item again against the disk. That first pass (before the audit row is updated) covers: not a symbolic link, no symbolic link above it, inside your home folder or `/Applications`, not itself a mount point, owned by you (a root-owned app is refused with a reason), size, entry count and no other volume mounted inside it, modification time unchanged since the scan, and fully measured. The cheap path checks (link, canonical path and letter case, owner, mount point, modification time) run a second time just before the move; the re-measure does not, because it can take seconds. The confirmation is also tied to whether you gave the second confirmation for yours items, and to the exact items that were shown as movable: an item that was refused in the preview is never moved by it. `HOME` must be the account's own home folder (a symbolic link to it is followed), a folder of yours at least two levels below the root. Rows a crash left unfinished are marked interrupted at the next start. An item labelled yours also needs `confirm_yours=1`. A request writes an audit row for each item to the history database before anything moves, and if that cannot be written nothing moves. The history database keeps the newest 10,000 such rows. `GET /api/history` needs no token, so it shows those paths, and any path quoted in an error text, with your home folder written as `~`. Limits: a file overwritten with the same length passes the change check, and a folder swapped for a link in the instant between the second check and the move is a race no check can close. The Trash keeps the item either way. One removal runs at a time and a request has a 45 second budget.

**Test coverage.** The mount-crossing refusals are verified on a developer's machine only. The two tests that attach a disk image are skipped in CI, because `hdiutil` has frozen a hosted runner (run them with `ML_RUN_MOUNT_TESTS=1`, see [commands](docs/agents/commands.md)). CI still covers the device comparison itself with a unit test.

`GET /api/storage` lists the names of app caches, the paths of developer projects that have a `node_modules` folder, and how big each is. It needs no token, only the Host check. The Host check and the browser's cross-origin rules stop a web page you open from reading the reply. They do not stop that page from sending the request, so a page could ask for a new scan. To limit that, a request starts a scan at most once a minute. Any process on your Mac can read the reply, because it can reach `127.0.0.1`. Cloud folders such as `~/Dropbox` are not user-content folders, so an idle `node_modules` inside one can be labelled safe.

## What the disk writes history holds

The `disk_writes_history` setting is off by default. While it is on, the history database keeps one row per app and hour: the app group's name and the bytes its processes wrote, for 7 days. It never stores a pid, a path or a file name, and it counts only your own processes. App names with hourly counts reveal habits, so `GET /api/storage/writes` needs the per-run token even though it only reads. Turning the setting on or off goes through `POST /api/settings`, and `POST /api/storage/writes/erase` deletes every row. All three need the Origin check and the per-run token.

The same reply carries the internal SSD's lifetime bytes written. That figure comes from the drive's NVMe SMART data, read through Apple's SMART plug-in without root. Where the drive does not offer it, the reply says why and the page hides the figure.

## What the baseline means

Marking an item as expected hides it from the Overview and the New filters. It does not make the item safe.

- A mark never raises a trust label. The rules still see every finding, so an app keeps the label it had before the mark.
- A mark on open ports covers exactly the ports you saw. A new port brings the finding back, and you mark it again.
- New hosts are marked host by host, so a new host is reported even after others were marked.
- An unsigned program using the network, and an open remote-debugging port, can never be marked. The server refuses the request with a 400.
- Marked findings stay listed in a "Marked as expected" section on the Overview, with an Unmark button. A marked host keeps its row with an "expected" chip.
- A mark on a startup item covers the version you saw: it records the item's fingerprint (its plist and its program's signing identity). If the plist or the program changes, the item is no longer expected, is reported again as changed, and shows as needing attention. An entry that is only a path, written by hand, matches every version and is not recommended.
- Every mark and unmark is written to the history database before it takes effect, and the History page lists them. If that write fails, nothing is changed. If the baseline file cannot then be written, a second row says so.
- The baseline holds at most 5,000 entries and the history keeps the newest 10,000 changes.
- A corrupt `baseline.json` is moved aside without overwriting older copies, and the Overview shows a banner saying earlier marks are not in effect. An unreadable one pauses marking until it can be read.
- Anyone who can write `~/.monitor-lizard/baseline.json` can hide things from you. That is the same access that is out of scope below.

## What is protected on disk

The data folder `~/.monitor-lizard` is meant to be mode 0700, and each file the app keeps in it 0600: the history database and its WAL and SHM files, the lock file, the backup, `baseline.json` and `settings.json`. New ones are created that way. Other local users then cannot read the sudo and su launch history.

The first time a process reads the data folder location, it also tightens an existing folder and its files. That covers folders made by an older version or copied from `~/.sys-monitor` or `~/.ownwatch`. It prints one line for a change and one for anything it could not do.

The pass changes a mode only when you own the entry and its mode is wider than the target. It skips symlinks, special files, and files with more than one hard link, and it counts them in the report. It never changes file contents.

## What is not covered

- Root and admin users can always read the folder.
- Earlier copies, such as Time Machine and backups made before the fix, keep their old modes.
- An ACL or the immutable flag can still grant access. The pass does not remove them.
- The mode of your home folder is not changed.
- A folder or file owned by someone else, for example after running with sudo, is not changed. The report names the folder and says how to fix it.
- The older folders `~/.sys-monitor` and `~/.ownwatch` are never changed.
- Files outside this folder are not covered. The in-app guide and the project files are public text, not secrets.
- If a mode change fails, the app says so once and carries on.

## Out of scope

Findings that need local root, or that assume the attacker already runs code as you and can read `~/.monitor-lizard/` (or an older `~/.sys-monitor/` or `~/.ownwatch/`, kept for migration).
