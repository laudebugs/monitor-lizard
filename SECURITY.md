# Security

Report vulnerabilities through GitHub's private vulnerability reporting (Security tab, "Report a vulnerability"). Do not open a public issue.

You can expect an acknowledgement within a few days. Say in the report what is affected and how to reproduce it.

## In scope

system-monitor can stop processes on your Mac, so these matter most:

- A way for a web page or another local process to stop a process, approve an app, or read data without the per-run token (CSRF, DNS rebinding, token leakage).
- A way to make it signal processes it refuses: root-owned, launchd, itself, or another user's.
- The server listening on anything other than 127.0.0.1.
- Injection through process names, command lines, or other attacker-controlled strings shown in the dashboard.

## Design guarantees

Loopback only, Host and Origin checks on every request, a random per-run token on every write, own-user processes only, an audit log of every action, no telemetry. Tests need no credentials and make no network calls.

## What the baseline means

Marking an item as expected hides it from the Overview and the New filters. It does not make the item safe.

- A mark never raises a trust label. The rules still see every finding, so an app keeps the label it had before the mark.
- A mark on open ports covers exactly the ports you saw. A new port brings the finding back, and you mark it again.
- New hosts are marked host by host, so a new host is reported even after others were marked.
- An unsigned program using the network, and an open remote-debugging port, can never be marked. The server refuses the request with a 400.
- Marked findings stay listed in a "Marked as expected" section on the Overview, with an Unmark button. A marked host or startup item keeps its row with an "expected" chip.
- Every mark and unmark is written to the history database before it takes effect, and the History page lists them. If that write fails, nothing is changed. If the baseline file cannot then be written, a second row says so.
- The baseline holds at most 5,000 entries and the history keeps the newest 10,000 changes.
- A corrupt `baseline.json` is moved aside without overwriting older copies, and the Overview shows a banner saying earlier marks are not in effect. An unreadable one pauses marking until it can be read.
- Anyone who can write `~/.monitor-lizard/baseline.json` can hide things from you. That is the same access that is out of scope below.

## Out of scope

Findings that need local root, or that assume the attacker already runs code as you and can read `~/.monitor-lizard/` (or an older `~/.sys-monitor/` or `~/.ownwatch/`, kept for migration).
