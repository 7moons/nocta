# Documentation

- [Architecture](architecture.md): layers, core modules, data and secrets, launching, IPC.
- [Operations](operations.md): data location, backups, the browser runtime, releases.
- [Testing](testing.md): every test suite and how to run it.
- [Feature parity](account-manager-parity.md): features taken over from Roblox Account Manager and the tests behind them.
- [Design](design.md): visual direction, layout, the launch workflow, themes and languages.

## Decisions

1. [Rust core, Tauri shell and React interface](adr/0001-rust-tauri-react.md)
2. [Windows only](adr/0002-windows-only.md)
3. [Independent storage with encrypted secrets](adr/0003-independent-local-storage.md)
4. [A Tauri-independent core and separate operation states](adr/0004-core-and-operation-lifecycle.md)
