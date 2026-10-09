# Changelog

All notable changes are recorded here. This project follows semantic versioning once it is tagged.

## [Unreleased] 0.1.0

Three permissionless contracts replace the earlier registries.

- `capsule_anchor`: ordered, hash-linked anchoring of capsule versions.
- `license_ledger`: grant and revoke records, with an `is_active` check.
- `usage_ledger`: gap-free, hash-linked usage receipts.
- No admin, no upgrade path and no fees; entries are namespaced by the writing account; every write extends storage lifetime.
- Deployed to Testnet; addresses are in `deployments/testnet.json`.
- Removed: `expert_registry`, `capsule_registry`, `license_registry`, `usage_registry` and `revenue_vault`, which assumed a hosted platform.
