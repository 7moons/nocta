# 2. Windows only

Status: accepted

## Context

Roblox has no official client for Linux, and much of Nocta depends on Windows: the launch handler,
process tracking with Job Objects, multi-instance, window control, Credential Manager and
WebView2. An earlier version also had macOS adapters, which were never tested on real Macs.

## Decision

Nocta supports Windows 10 and 11 only. Building for another system stops with a compile error.
Releases are an installer and a portable package.

## Consequences

Code needs no platform branches, and every path is the one users run. Supporting macOS again would
mean new platform adapters and testing on real Macs.
