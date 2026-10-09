# synapse-contracts working rules

- Contracts here are permissionless: no admin, no owner, no upgrade path, no fees. Do not add any.
- Every write takes the writing account first and calls `require_auth()` on it; state is keyed by that account. Never key state in a way that lets one account affect another's entries.
- Every write must extend the TTL of the entries it touches (and the instance). Add a test for it using `testutils::storage::{Instance as _, Persistent as _}`.
- Store hashes, counts and times only. Never store questions, answers, capsule content or personal data.
- Keep the references (`capsule_ref`, `license_ref`) derivations identical to `synapse-core/SPEC.md` section 9; change both or neither.
- Keep repository boundaries in `docs/REPOSITORIES.md` of synapse-core; do not copy format or licensing rules into the contracts.
- Add tests for each rejected path (every `Error` variant) as well as the accepted ones.
- Do not put credentials, seeds, tokens or real customer data in this repository.
