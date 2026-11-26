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

## Out of scope

Findings that need local root, or that assume the attacker already runs code as you and can read `~/.monitor-lizard/` (or an older `~/.sys-monitor/` or `~/.ownwatch/`, kept for migration).
