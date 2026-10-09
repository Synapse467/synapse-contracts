#![no_std]
//! Anchors the hash of each published version of an expertise capsule, as a tamper-evident chain.
//!
//! There is no admin and no allow-list. Every record lives under the address that wrote it, and
//! only that address can write there, so one public deployment serves everyone.
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
};

// Soroban archives a persistent entry when its TTL runs out. Every write renews what it touches:
// once under ~30 days remain, extend to ~150 days (17,280 ledgers a day), below the network
// ceiling of 3,110,400 ledgers.
const TTL_THRESHOLD: u32 = 518_400;
const TTL_EXTEND_TO: u32 = 2_592_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InvalidVersion = 1,
    VersionExists = 2,
    VersionGap = 3,
    BrokenChain = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Anchor {
    pub manifest_hash: BytesN<32>,
    pub previous_hash: BytesN<32>,
    pub anchored_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Anchor(Address, BytesN<32>, u32),
    Latest(Address, BytesN<32>),
}

#[contractevent(topics = ["capsule_anchored"])]
pub struct CapsuleAnchored {
    #[topic]
    pub owner: Address,
    #[topic]
    pub capsule_ref: BytesN<32>,
    pub version: u32,
    pub manifest_hash: BytesN<32>,
}

#[contract]
pub struct CapsuleAnchor;

#[contractimpl]
impl CapsuleAnchor {
    // Records version `version` of the capsule `capsule_ref` owned by `owner`. Versions must be
    // 1, 2, 3, ... with no gaps, and each must name the hash of the one before it (all zeros for
    // version 1), so history cannot be rewritten or branched. A plain `//` comment is used
    // deliberately: `///` on a `#[contractimpl]` method is captured into the contract spec.
    pub fn anchor(
        env: Env,
        owner: Address,
        capsule_ref: BytesN<32>,
        version: u32,
        manifest_hash: BytesN<32>,
        previous_hash: BytesN<32>,
    ) -> Result<(), Error> {
        owner.require_auth();
        if version == 0 {
            return Err(Error::InvalidVersion);
        }
        let store = env.storage().persistent();
        let key = DataKey::Anchor(owner.clone(), capsule_ref.clone(), version);
        if store.has(&key) {
            return Err(Error::VersionExists);
        }
        let latest_key = DataKey::Latest(owner.clone(), capsule_ref.clone());
        let latest: u32 = store.get(&latest_key).unwrap_or(0);
        if latest.checked_add(1) != Some(version) {
            return Err(Error::VersionGap);
        }
        let expected_previous = if latest == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let previous: Anchor = store
                .get(&DataKey::Anchor(owner.clone(), capsule_ref.clone(), latest))
                .ok_or(Error::BrokenChain)?;
            previous.manifest_hash
        };
        if previous_hash != expected_previous {
            return Err(Error::BrokenChain);
        }
        store.set(
            &key,
            &Anchor {
                manifest_hash: manifest_hash.clone(),
                previous_hash,
                anchored_at: env.ledger().timestamp(),
            },
        );
        store.set(&latest_key, &version);
        store.extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        store.extend_ttl(&latest_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        CapsuleAnchored {
            owner,
            capsule_ref,
            version,
            manifest_hash,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get(env: Env, owner: Address, capsule_ref: BytesN<32>, version: u32) -> Option<Anchor> {
        env.storage()
            .persistent()
            .get(&DataKey::Anchor(owner, capsule_ref, version))
    }

    pub fn latest(env: Env, owner: Address, capsule_ref: BytesN<32>) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Latest(owner, capsule_ref))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger},
        Event,
    };

    fn hash(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }
    fn zero(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0; 32])
    }
    fn setup() -> (Env, CapsuleAnchorClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(CapsuleAnchor, ());
        let client = CapsuleAnchorClient::new(&env, &id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn anchors_a_chain_of_versions() {
        let (env, client, owner) = setup();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        let capsule = hash(&env, 1);
        assert_eq!(client.latest(&owner, &capsule), 0);
        client.anchor(&owner, &capsule, &1, &hash(&env, 10), &zero(&env));
        client.anchor(&owner, &capsule, &2, &hash(&env, 20), &hash(&env, 10));
        client.anchor(&owner, &capsule, &3, &hash(&env, 30), &hash(&env, 20));
        assert_eq!(client.latest(&owner, &capsule), 3);
        let second = client.get(&owner, &capsule, &2).unwrap();
        assert_eq!(second.manifest_hash, hash(&env, 20));
        assert_eq!(second.previous_hash, hash(&env, 10));
        assert_eq!(second.anchored_at, 1_000);
        assert!(client.get(&owner, &capsule, &4).is_none());
    }

    #[test]
    fn emits_an_event() {
        let (env, client, owner) = setup();
        let capsule = hash(&env, 1);
        let id = client.address.clone();
        client.anchor(&owner, &capsule, &1, &hash(&env, 10), &zero(&env));
        assert_eq!(
            env.events().all(),
            std::vec![CapsuleAnchored {
                owner: owner.clone(),
                capsule_ref: capsule.clone(),
                version: 1,
                manifest_hash: hash(&env, 10)
            }
            .to_xdr(&env, &id)]
        );
    }

    #[test]
    fn a_published_version_can_never_be_replaced() {
        let (env, client, owner) = setup();
        let capsule = hash(&env, 1);
        client.anchor(&owner, &capsule, &1, &hash(&env, 10), &zero(&env));
        assert_eq!(
            client.try_anchor(&owner, &capsule, &1, &hash(&env, 99), &zero(&env)),
            Err(Ok(Error::VersionExists))
        );
        assert_eq!(
            client.get(&owner, &capsule, &1).unwrap().manifest_hash,
            hash(&env, 10)
        );
    }

    #[test]
    fn rejects_version_zero_gaps_and_wrong_links() {
        let (env, client, owner) = setup();
        let capsule = hash(&env, 1);
        assert_eq!(
            client.try_anchor(&owner, &capsule, &0, &hash(&env, 10), &zero(&env)),
            Err(Ok(Error::InvalidVersion))
        );
        // The first version must be 1, not 2.
        assert_eq!(
            client.try_anchor(&owner, &capsule, &2, &hash(&env, 10), &zero(&env)),
            Err(Ok(Error::VersionGap))
        );
        // The first version must link to the zero hash.
        assert_eq!(
            client.try_anchor(&owner, &capsule, &1, &hash(&env, 10), &hash(&env, 5)),
            Err(Ok(Error::BrokenChain))
        );
        client.anchor(&owner, &capsule, &1, &hash(&env, 10), &zero(&env));
        // A later version must link to the previous version's hash.
        assert_eq!(
            client.try_anchor(&owner, &capsule, &2, &hash(&env, 20), &hash(&env, 11)),
            Err(Ok(Error::BrokenChain))
        );
        assert_eq!(client.latest(&owner, &capsule), 1);
    }

    #[test]
    fn owners_cannot_write_into_each_others_namespace() {
        let (env, client, alice) = setup();
        let bob = Address::generate(&env);
        let capsule = hash(&env, 1);
        client.anchor(&alice, &capsule, &1, &hash(&env, 10), &zero(&env));
        // Bob anchors the same capsule reference under his own address: independent history.
        client.anchor(&bob, &capsule, &1, &hash(&env, 77), &zero(&env));
        assert_eq!(
            client.get(&alice, &capsule, &1).unwrap().manifest_hash,
            hash(&env, 10)
        );
        assert_eq!(
            client.get(&bob, &capsule, &1).unwrap().manifest_hash,
            hash(&env, 77)
        );
    }

    #[test]
    #[should_panic]
    fn an_unauthorized_anchor_is_rejected() {
        let env = Env::default();
        let id = env.register(CapsuleAnchor, ());
        CapsuleAnchorClient::new(&env, &id).anchor(
            &Address::generate(&env),
            &hash(&env, 1),
            &1,
            &hash(&env, 10),
            &zero(&env),
        );
    }

    #[test]
    fn every_write_extends_the_storage_lifetime() {
        let (env, client, owner) = setup();
        let capsule = hash(&env, 1);
        client.anchor(&owner, &capsule, &1, &hash(&env, 10), &zero(&env));
        let id = client.address.clone();
        let entry = || {
            env.as_contract(&id, || {
                env.storage().persistent().get_ttl(&DataKey::Anchor(
                    owner.clone(),
                    capsule.clone(),
                    1,
                ))
            })
        };
        let instance = || env.as_contract(&id, || env.storage().instance().get_ttl());
        assert_eq!(entry(), TTL_EXTEND_TO);
        assert_eq!(instance(), TTL_EXTEND_TO);
    }
}
