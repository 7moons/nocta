//! IPC commands the frontend can call, grouped by area. Every command is registered
//! explicitly in `lib.rs`; the frontend has no other way into the backend.
pub mod accounts;
pub mod automation;
pub mod client;
pub mod launch;
pub mod legacy;
pub mod social;
pub mod workspace;
