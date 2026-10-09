# Security

## Reporting a vulnerability

Please report vulnerabilities privately, through GitHub's **Report a vulnerability** button on this repository's Security tab. Do not open a public issue for something exploitable.

You will get an acknowledgement, and we will agree a disclosure date with you once the fix is ready.

## What counts

- Any way for one account to write, overwrite, revoke or block another account's entries.
- A function that only some privileged account can call. The contracts have no admin by design.
- A way to break the order or hash-linking of anchors or usage receipts.
- Entries that are not extended on write and can expire unexpectedly because of a bug.

## What does not

- Expiry of entries that nobody has written to or extended for a long time. This is Soroban's storage model, and it is documented in `docs/CONTRACTS.md`.
- Testnet being reset.

## Notes

The contracts store only hashes, counts and times. They hold no funds and have no upgrade path, so the main risk is to the integrity of the record, not to money. A Mainnet deployment is a separate step with its own review.
