<p align="center"><img src="assets/logo.svg" alt="Synapse logo" width="112"></p>

<h1 align="center">synapse-contracts</h1>

<p align="center"><b>Three permissionless Soroban contracts that anchor capsule versions, licenses and usage on Stellar.</b></p>

<p align="center">
  <a href="https://github.com/Synapse467/synapse-contracts/actions/workflows/ci.yml"><img src="https://github.com/Synapse467/synapse-contracts/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/Synapse467/synapse-contracts/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Synapse467/synapse-contracts?color=blue" alt="License: MIT"></a>
  <a href="https://github.com/Synapse467/synapse-contracts/releases"><img src="https://img.shields.io/github/v/release/Synapse467/synapse-contracts?color=brightgreen" alt="Latest release"></a>
  <a href="https://github.com/Synapse467/synapse-contracts/issues"><img src="https://img.shields.io/github/issues/Synapse467/synapse-contracts?color=orange" alt="Open issues"></a>
  <a href="https://github.com/Synapse467/synapse-contracts/issues?q=is%3Aopen+label%3A%22help+wanted%22"><img src="https://img.shields.io/badge/help%20wanted-welcome-8A2BE2" alt="Help wanted"></a>
  <img src="https://img.shields.io/badge/built%20for-Stellar-black" alt="Built for Stellar">
</p>

<p align="center">
  <a href="https://cjay-1.gitbook.io/synapse-docs/">Documentation</a> ·
  <a href="https://github.com/Synapse467/synapse-contracts/releases">Releases</a> ·
  <a href="https://github.com/Synapse467/synapse-contracts/issues">Issues</a> ·
  <a href="CONTRIBUTING.md">Contributing</a> ·
  <a href="SECURITY.md">Security</a>
</p>

---


Three small Soroban contracts that give [Synapse](https://github.com/Synapse467/synapse-cli/blob/main/docs/overview.md) an independent public record on Stellar: **which version of a capsule existed, who licensed it, and how it was used.**

Synapse works without them. Capsules, licenses and usage logs are files that are verified offline. These contracts exist so that three things can be proved to a stranger without asking the owner:

| Contract | What it records | Why anyone would care |
|---|---|---|
| [`capsule_anchor`](contracts/capsule_anchor) | The hash of each published version of a capsule, in order | An owner cannot quietly swap the file after you bought it. A buyer can check the file they hold against the anchor. |
| [`license_ledger`](contracts/license_ledger) | That a license was granted, and whether it has been revoked | Revocation becomes public and immediate for anyone who checks, not just for people the owner can reach. |
| [`usage_ledger`](contracts/usage_ledger) | Sealed batches of usage: a hash, a count and a period | A gap-free, hash-linked receipt trail that both sides can show, without revealing a single question. |

## Design: no admin, no registry, no tokens

The contracts have **no owner, no admin, no upgrade path and no fees**. Nobody deploys "their own copy": everyone uses the same deployment, and every entry is namespaced by the account that wrote it.

- A record is keyed by `(account, reference)`. Only that account can write or read-modify it (`require_auth`), so there is nothing to impersonate and nothing to configure.
- References are 32-byte hashes derived from content (`sha256("synapse.capsule\n" + owner + "\n" + slug)`), so two owners can use the same slug without colliding.
- Nothing personal is stored: hashes, counts and timestamps only. Questions, answers and capsule contents never touch the chain.
- Writes extend the storage lifetime of everything they touch (see [Storage lifetime](docs/CONTRACTS.md#storage-lifetime)), so records an owner keeps writing to do not expire.

This is why Synapse needs no server, account system or configuration: the chain is a shared, neutral notary that the command line talks to directly.

## Deployment

Public Testnet deployment, used by default by [`synapse-core`](https://github.com/Synapse467/synapse-core):

| Contract | Address |
|---|---|
| `capsule_anchor` | `CAII7IQVEDGYE3VMO4JZA2V7JMPSHIISJFBX2LWV7XPAP5GQUL6UPXQP` |
| `license_ledger` | `CAI26T22K4EU4M6OQA7RJUAV5PQZYFJQJIIAPW2FUXJCQ3ADAWQMBMFN` |
| `usage_ledger` | `CDNVTWXTSQ66D34KKCBUIESF2OSA67L7WU7SWOGSLI7IYONVHLF7XHKX` |

The same data is in [`deployments/testnet.json`](deployments/testnet.json). Testnet is for development: it is reset periodically and its funds have no value. A Mainnet deployment is a deliberate, separate step and is not part of this release.

## Build and test

```bash
cargo test                                   # all three contracts
cargo build --release --target wasm32v1-none # the deployable WASM
```

On Windows with the GNU toolchain, test binaries can exceed the exported-symbol limit; run `RUSTFLAGS="-C link-arg=-Wl,--exclude-all-symbols" cargo test`.

## Using the contracts

Almost nobody should call them directly. The Synapse command line does it for you, creating and funding an account on first use:

```bash
synapse chain anchor my-capsule.capsule.json   # timestamp a version
synapse chain status my-capsule.capsule.json   # check a file against its anchor
```

To call them from your own code, use `synapse-core`'s `chain` package (Go), or invoke the contracts with any Soroban SDK. The full interface, error codes and events are in [`docs/CONTRACTS.md`](docs/CONTRACTS.md).

## Repositories

This is one of four repositories; see [`../REPOSITORIES.md`](https://github.com/Synapse467/synapse-core/blob/main/docs/REPOSITORIES.md) for how they fit together.

| Repository | Role |
|---|---|
| **synapse-contracts** (this one) | The on-chain record |
| [synapse-core](https://github.com/Synapse467/synapse-core) | The formats and rules: capsules, licenses, signatures, usage logs, and the Stellar client |
| [synapse-engine](https://github.com/Synapse467/synapse-engine) | Answering questions from a capsule, extracting knowledge, evaluating a capsule, the HTTP gateway and the MCP server |
| [synapse-cli](https://github.com/Synapse467/synapse-cli) | The `synapse` command |

## License

MIT. See [LICENSE](LICENSE). Security reports: see [SECURITY.md](SECURITY.md).

## Maintainers

| Maintainer | Role | Contact |
| --- | --- | --- |
| [Synapse467](https://github.com/Synapse467) | Organization owner, releases | [GitHub issues](https://github.com/Synapse467/synapse-contracts/issues) |

## Community

Ask questions and propose changes in [GitHub issues](https://github.com/Synapse467/synapse-contracts/issues). Read the [documentation](https://cjay-1.gitbook.io/synapse-docs/) first; the [FAQ](https://cjay-1.gitbook.io/synapse-docs/project/faq) answers the common questions.

## Contributors

<a href="https://github.com/Synapse467/synapse-contracts/graphs/contributors"><img src="https://contrib.rocks/image?repo=Synapse467/synapse-contracts" alt="Contributors"></a>
