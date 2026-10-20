# 1. Rust core, Tauri shell and React interface

Status: accepted

## Context

The previous Nocta was an Electron app. It was rewritten from scratch with the same features,
delivered in stages.

## Decision

Domain logic is written in Rust, the desktop shell in Tauri 2 and the interface in React with
TypeScript. React was chosen over the alternatives for its ecosystem for large tables and because
the existing workflows were already designed around it. Bun runs the frontend tooling and scripts;
Cargo builds the Rust side.

## Consequences

Desktop logic moved from Node to Rust. Chromium automation, launching and multi-instance support
had to be built and verified per platform; the framework choice does not provide them. Linux was
later dropped (ADR 2).
