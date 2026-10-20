# 3. Independent storage with encrypted secrets

Status: accepted; amended by the old-database importer

## Context

The previous app stored accounts, passwords and cookies in a plain SQLite database that could be
reopened from several historical locations.

## Decision

The new app starts with its own empty database. Secrets are encrypted with AES-256-GCM using a key
kept in Windows Credential Manager, so no password is asked at startup. Moving data between computers goes
through a password-protected backup (key derived with Argon2id); a copy of the data folder alone
does not carry the key.

The old database is never opened by the app on its own. Later, an explicit importer was added
(Settings → Backup & import): it finds old databases in their usual folders, reads them read-only
and imports accounts and folders into the new, encrypted store when the user asks.

## Consequences

Old data is never modified. Restarting reads secrets back through Credential Manager.
