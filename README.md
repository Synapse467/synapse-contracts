# Synapse Smart Contracts (Stellar / Soroban)

Soroban smart contracts workspace for the **Synapse** platform, anchoring expertise ownership, version immutability, access licensing, usage verification, and revenue settlements on Stellar.

## Contracts Overview

In accordance with PRD Sections 17, 21, and 22, private knowledge and source material remain strictly off-chain. Only cryptographic hashes and proofs are recorded on-chain:

1. **`expert_registry`**:
   - Manages expert registration via opaque hashes (`expert_ref`, `controller`, `metadata_hash`).
   - Tracks verification status transitions (`Unverified`, `Pending`, `Verified`, `Revoked`).
   - Invariant: Zero PII or resume details stored on-chain.

2. **`capsule_registry`**:
   - Registers capsules and publishes version manifests (`manifest_hash`, `eval_hash`).
   - Invariant: Published version entries are strictly immutable (attempting to overwrite an existing version errors out).

3. **`license_registry`**:
   - Issues cryptographic access grants (`grantee`, `terms_hash`, validity window).
   - Licensor-authorized revocation.
   - Enforces active status check: `is_active(license_ref, now) -> bool`.

4. **`usage_receipt_registry`**:
   - Records verifiable usage batch manifests (`usage_manifest_hash`, `period`).
   - Authorized by recorder / platform authority.
   - Invariant: Zero raw queries, identities, or responses stored on-chain.

5. **`settlement`**:
   - Disburses revenue splits based on basis points (`share_bps`).
   - Enforces 100% (10,000 bps) allocation invariants and guarantees zero rounding loss.

## Building and Testing

### Prerequisites
- Rust 1.84+ (tested on Rust 1.97)
- `wasm32v1-none` target (`rustup target add wasm32v1-none`)

### Host Unit Tests
```bash
cargo test --workspace
```

Every state-changing method on all 5 contracts requires `require_auth()` on the correct principal, and every contract has a dedicated test proving an unauthorized caller (no matching signature/mock) is rejected, in addition to its happy-path test — 10 tests total. `cargo fmt --check` and `cargo check --workspace` are clean.

A real Testnet deployment (public contract IDs, deployer public key, and tx hashes only — no private keys) is recorded in `deployments/testnet.json` and was used to verify `capsule_registry` end-to-end on-chain.

### WASM Compilation
To compile optimized WASM bytecode for Soroban on-chain deployment:
```bash
cargo rustc --package expert_registry --target wasm32v1-none --release --crate-type cdylib
cargo rustc --package capsule_registry --target wasm32v1-none --release --crate-type cdylib
cargo rustc --package license_registry --target wasm32v1-none --release --crate-type cdylib
cargo rustc --package usage_receipt_registry --target wasm32v1-none --release --crate-type cdylib
cargo rustc --package settlement --target wasm32v1-none --release --crate-type cdylib
```
