# Security

## Reporting a vulnerability

Please do not open a public issue for a vulnerability. Report it privately through
[GitHub security advisories](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository, with the steps to reproduce it and what an attacker could gain.

## How Nocta handles secrets

Nocta stores account passwords, session cookies and two-step verification keys. The design rules:

- Secrets are encrypted with AES-256-GCM before they reach the database, with a fresh nonce per
  value and the record it belongs to as associated data. The key is random per installation and
  kept in Windows Credential Manager, never next to the database.
- The interface never receives a stored secret. Snapshots and IPC responses contain metadata only;
  copying accounts to the clipboard and backups are the only explicit ways secrets leave the store.
- Backups are encrypted with a key derived from your password (Argon2id).
- Cookies are only sent to HTTPS `roblox.com` hosts. Every redirect is checked again before it is
  followed, response sizes and times are limited, and errors are sanitized before they are logged.
- Private server access codes stay in the backend behind opaque, expiring IDs.
- Types that hold secrets redact `Debug` output and are zeroed when dropped.
- Managed browsers use fresh profiles in Nocta's own data folder; the user's own Chrome profile is
  never touched.
