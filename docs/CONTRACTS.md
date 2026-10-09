# Contract reference

All three contracts follow the same rules:

- **The writer is the namespace.** Every write takes the writing account as its first argument and calls `require_auth()` on it. State is stored under that account, so one account can never read-modify, overwrite or block another's entries.
- **Hashes are `BytesN<32>`.** Capsule and license references are derived off-chain (see below); the contracts treat them as opaque.
- **Writes are append-only or one-way.** An anchor cannot be replaced, a revoked license cannot be un-revoked, and usage receipts cannot be edited.
- **No admin and no upgrade.** There is no function that only a deployer can call.

Times are ledger timestamps (Unix seconds).

## References

| Reference | Derivation |
|---|---|
| Capsule ref | `sha256("synapse.capsule\n" + ownerAddress + "\n" + slug)` |
| License ref | `sha256("synapse.license\n" + licenseId)` where `licenseId` is the license's 128-bit hex ID |
| Manifest hash | `sha256` of the capsule manifest's canonical JSON (see `synapse-core` `SPEC.md`) |

## `capsule_anchor`

Records the hash of each published version of a capsule, as a chain.

### `anchor(owner, capsule_ref, version, manifest_hash, previous_hash)`

Anchors one version. Requires `owner`'s authorization.

- `version` starts at 1 and must be exactly one more than the latest anchored version (no gaps, no re-anchoring).
- `previous_hash` must equal the manifest hash anchored for `version - 1`; for version 1 it must be 32 zero bytes.

Errors: `InvalidVersion (1)` for version 0; `VersionExists (2)`; `VersionGap (3)`; `BrokenChain (4)`.
Event: `capsule_anchored`.

### `get(owner, capsule_ref, version) -> Option<Anchor>`

Returns `Anchor { manifest_hash, previous_hash, anchored_at }`, or none.

### `latest(owner, capsule_ref) -> u32`

The newest anchored version, or 0.

## `license_ledger`

Records that a license was granted, and when it was revoked.

### `grant(grantor, license_ref, capsule_ref, grantee, terms_hash, expires_at)`

Records a license. Requires `grantor`'s authorization. `terms_hash` is the license's hash, so a holder can show that the file they hold is the one that was granted. `expires_at` is a ledger time, or 0 for no expiry.

Errors: `AlreadyExists (1)`; `InvalidExpiry (4)` if `expires_at` is non-zero and not in the future.
Event: `license_granted`.

### `revoke(grantor, license_ref)`

Marks a license revoked and stamps `revoked_at`. Only the grantor who granted it can revoke it, because the entry is stored under the grantor.

Errors: `NotFound (2)`; `AlreadyRevoked (3)`.
Event: `license_revoked`.

### `get(grantor, license_ref) -> Option<License>`

`License { capsule_ref, grantee, terms_hash, granted_at, expires_at, revoked, revoked_at }`.

### `is_active(grantor, license_ref) -> bool`

True if the license exists, is not revoked, and has not expired by the current ledger time.

## `usage_ledger`

Records sealed batches of usage as a gap-free, hash-linked chain per license.

### `record(owner, license_ref, seq, batch_hash, previous_hash, count, period_start, period_end)`

Records one batch. Requires `owner`'s authorization.

- `seq` starts at 1 and must follow the last recorded `seq`.
- `previous_hash` must equal the last recorded batch hash; for `seq` 1 it must be 32 zero bytes.
- `count` must be at least 1, and `period_start <= period_end`.

Errors: `EmptyBatch (1)`; `InvalidPeriod (2)`; `OutOfOrder (3)`; `BrokenChain (4)`.
Event: `usage_recorded`.

A batch hash commits to the hashes of the individual usage events (`synapse-core` `usage` package), so the underlying log can later prove exactly which events a receipt covers without publishing them.

### `get(owner, license_ref, seq) -> Option<Receipt>` and `receipts(owner, license_ref) -> u32`

Read a receipt, and count how many are recorded.

## Storage lifetime

Soroban deletes persistent entries that nobody extends. Each write here extends the entries it touches (and the contract instance) to roughly 150 days (`TTL_EXTEND_TO = 2_592_000` ledgers at about five seconds each) whenever less than about 30 days (`518_400` ledgers) remain. Both are below the network maximum (3,110,400 ledgers on Testnet).

Consequences:

- An owner who keeps using a capsule, license or usage chain keeps its records alive as a side effect.
- Records that are never touched again can expire. Anyone can extend or restore them with Soroban's standard `ExtendFootprintTtl` and `RestoreFootprint` operations, and `synapse-core` handles restoration automatically when a write needs it.
- For records that must outlive their owner's activity, extend them as part of your own publishing routine.

## Verifying a deployment

```bash
stellar contract info interface --id <contract-id> --network testnet
```

compares the on-chain interface with the functions above.
