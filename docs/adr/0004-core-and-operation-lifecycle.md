# 4. A Tauri-independent core and separate operation states

Status: accepted

## Context

Nocta works with accounts, browser sessions and running game clients at the same time. Mixing their
states (an account's session result, the operation running on it, its running game) made the old
app hard to reason about.

## Decision

The core is a Rust library independent of Tauri, built on Tokio, reqwest, SQLite through SQLx,
serde, thiserror and tracing, and it generates the TypeScript types for the interface. An
account's session state, its task and its game instances are separate states. Conflicting
operations on one account are refused.

Stop cancels the queue and local work and closes task browsers, but leaves games and browsers that
wait for the user. Quitting finishes or cancels every task, closes all managed browsers, waits for
writes in progress and does not stay in the tray.

## Consequences

The interface owns neither processes nor secrets, and the core is tested without a desktop. A
request Roblox has already accepted cannot be undone by cancelling locally.
