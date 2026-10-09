# Contributing

Thank you for helping. A few things keep Synapse trustworthy.

## Before you start

- Read `AGENTS.md` (the rules every change follows).
- The contracts are permissionless and immutable by design. Open an issue before proposing anything that adds an admin, a fee, an upgrade path or shared state.
- The reference derivations (`capsule_ref`, `license_ref`) are defined in synapse-core's `SPEC.md`; a change here needs a matching change there.

## Build and test

Rust (stable) with the `wasm32v1-none` target.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --target wasm32v1-none
```

On Windows with the GNU toolchain: `RUSTFLAGS="-C link-arg=-Wl,--exclude-all-symbols" cargo test`.

## What a good change has

- A test for every accepted path and every `Error` variant.
- A test that each write extends the storage lifetime of what it touches.
- `docs/CONTRACTS.md` updated in the same change.
- No secrets, seeds or tokens anywhere.
- A note in `CHANGELOG.md`.
