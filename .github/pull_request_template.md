## What changes for the user

## How it was checked

- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p nocta-core`
- [ ] `bun run lint`, `bun run format:check`, `bun run build`, `bun run test`
- [ ] `bun run contracts` leaves `src/generated/contracts.ts` unchanged (or the change is committed)
- [ ] New text is in every locale
- [ ] `CHANGELOG.md` has an entry under the next version
- [ ] Screenshot attached for visible changes
