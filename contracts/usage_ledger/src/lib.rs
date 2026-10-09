#![no_std]
//! A tamper-evident chain of usage receipts for a license.
//!
//! A receipt commits to a batch of usage (a hash, a count and a period). The usage itself, and
//! above all the questions asked, never go on-chain. Receipts for one license are numbered 1, 2,
//! 3, ... without gaps and each names the hash of the one before it, so a batch cannot be dropped
//! or reordered. There is no admin: receipts live under the address that wrote them.
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
};

// Renew storage on every write (see capsule_anchor for the reasoning).
const TTL_THRESHOLD: u32 = 518_400;
const TTL_EXTEND_TO: u32 = 2_592_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EmptyBatch = 1,
    InvalidPeriod = 2,
    OutOfOrder = 3,
    BrokenChain = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub batch_hash: BytesN<32>,
    pub previous_hash: BytesN<32>,
    pub count: u32,
    pub period_start: u64,
    pub period_end: u64,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Receipt(Address, BytesN<32>, u32),
    Count(Address, BytesN<32>),
}

#[contractevent(topics = ["usage_recorded"])]
pub struct UsageRecorded {
    #[topic]
    pub owner: Address,
    #[topic]
    pub license_ref: BytesN<32>,
    pub seq: u32,
    pub batch_hash: BytesN<32>,
}

#[contract]
pub struct UsageLedger;

#[contractimpl]
impl UsageLedger {
    // Seven fields describe one receipt; Soroban takes each as its own argument.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        env: Env,
        owner: Address,
        license_ref: BytesN<32>,
        seq: u32,
        batch_hash: BytesN<32>,
        previous_hash: BytesN<32>,
        count: u32,
        period_start: u64,
        period_end: u64,
    ) -> Result<(), Error> {
        owner.require_auth();
        if count == 0 {
            return Err(Error::EmptyBatch);
        }
        if period_end < period_start {
            return Err(Error::InvalidPeriod);
        }
        let store = env.storage().persistent();
        let count_key = DataKey::Count(owner.clone(), license_ref.clone());
        let last: u32 = store.get(&count_key).unwrap_or(0);
        if last.checked_add(1) != Some(seq) {
            return Err(Error::OutOfOrder);
        }
        let expected_previous = if last == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let previous: Receipt = store
                .get(&DataKey::Receipt(owner.clone(), license_ref.clone(), last))
                .ok_or(Error::BrokenChain)?;
            previous.batch_hash
        };
        if previous_hash != expected_previous {
            return Err(Error::BrokenChain);
        }
        let key = DataKey::Receipt(owner.clone(), license_ref.clone(), seq);
        store.set(
            &key,
            &Receipt {
                batch_hash: batch_hash.clone(),
                previous_hash,
                count,
                period_start,
                period_end,
                recorded_at: env.ledger().timestamp(),
            },
        );
        store.set(&count_key, &seq);
        store.extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        store.extend_ttl(&count_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        UsageRecorded {
            owner,
            license_ref,
            seq,
            batch_hash,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get(env: Env, owner: Address, license_ref: BytesN<32>, seq: u32) -> Option<Receipt> {
        env.storage()
            .persistent()
            .get(&DataKey::Receipt(owner, license_ref, seq))
    }

    pub fn receipts(env: Env, owner: Address, license_ref: BytesN<32>) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Count(owner, license_ref))
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
    fn setup() -> (Env, UsageLedgerClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 9_000);
        let id = env.register(UsageLedger, ());
        let client = UsageLedgerClient::new(&env, &id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn records_a_numbered_hash_linked_chain() {
        let (env, client, owner) = setup();
        let license = hash(&env, 1);
        assert_eq!(client.receipts(&owner, &license), 0);
        client.record(
            &owner,
            &license,
            &1,
            &hash(&env, 10),
            &zero(&env),
            &5,
            &100,
            &200,
        );
        client.record(
            &owner,
            &license,
            &2,
            &hash(&env, 20),
            &hash(&env, 10),
            &3,
            &200,
            &300,
        );
        assert_eq!(client.receipts(&owner, &license), 2);
        let second = client.get(&owner, &license, &2).unwrap();
        assert_eq!(second.batch_hash, hash(&env, 20));
        assert_eq!(second.previous_hash, hash(&env, 10));
        assert_eq!(
            (second.count, second.period_start, second.period_end),
            (3, 200, 300)
        );
        assert_eq!(second.recorded_at, 9_000);
        assert!(client.get(&owner, &license, &3).is_none());
    }

    #[test]
    fn rejects_gaps_replays_and_broken_links() {
        let (env, client, owner) = setup();
        let license = hash(&env, 1);
        // The first receipt must be number 1.
        assert_eq!(
            client.try_record(
                &owner,
                &license,
                &2,
                &hash(&env, 10),
                &zero(&env),
                &1,
                &0,
                &1
            ),
            Err(Ok(Error::OutOfOrder))
        );
        client.record(
            &owner,
            &license,
            &1,
            &hash(&env, 10),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        // The same number cannot be recorded twice.
        assert_eq!(
            client.try_record(
                &owner,
                &license,
                &1,
                &hash(&env, 11),
                &zero(&env),
                &1,
                &0,
                &1
            ),
            Err(Ok(Error::OutOfOrder))
        );
        // A receipt must name the hash of the one before it.
        assert_eq!(
            client.try_record(
                &owner,
                &license,
                &2,
                &hash(&env, 20),
                &hash(&env, 99),
                &1,
                &1,
                &2
            ),
            Err(Ok(Error::BrokenChain))
        );
        assert_eq!(client.receipts(&owner, &license), 1);
    }

    #[test]
    fn rejects_empty_batches_and_backwards_periods() {
        let (env, client, owner) = setup();
        let license = hash(&env, 1);
        assert_eq!(
            client.try_record(
                &owner,
                &license,
                &1,
                &hash(&env, 10),
                &zero(&env),
                &0,
                &0,
                &1
            ),
            Err(Ok(Error::EmptyBatch))
        );
        assert_eq!(
            client.try_record(
                &owner,
                &license,
                &1,
                &hash(&env, 10),
                &zero(&env),
                &1,
                &5,
                &4
            ),
            Err(Ok(Error::InvalidPeriod))
        );
    }

    #[test]
    fn chains_are_separate_per_owner_and_per_license() {
        let (env, client, alice) = setup();
        let bob = Address::generate(&env);
        client.record(
            &alice,
            &hash(&env, 1),
            &1,
            &hash(&env, 10),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        client.record(
            &alice,
            &hash(&env, 2),
            &1,
            &hash(&env, 11),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        client.record(
            &bob,
            &hash(&env, 1),
            &1,
            &hash(&env, 12),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        assert_eq!(client.receipts(&alice, &hash(&env, 1)), 1);
        assert_eq!(client.receipts(&alice, &hash(&env, 2)), 1);
        assert_eq!(client.receipts(&bob, &hash(&env, 1)), 1);
    }

    #[test]
    fn emits_an_event() {
        let (env, client, owner) = setup();
        let license = hash(&env, 1);
        let id = client.address.clone();
        client.record(
            &owner,
            &license,
            &1,
            &hash(&env, 10),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        assert_eq!(
            env.events().all(),
            std::vec![UsageRecorded {
                owner: owner.clone(),
                license_ref: license.clone(),
                seq: 1,
                batch_hash: hash(&env, 10)
            }
            .to_xdr(&env, &id)]
        );
    }

    #[test]
    #[should_panic]
    fn an_unauthorized_record_is_rejected() {
        let env = Env::default();
        let id = env.register(UsageLedger, ());
        UsageLedgerClient::new(&env, &id).record(
            &Address::generate(&env),
            &hash(&env, 1),
            &1,
            &hash(&env, 10),
            &zero(&env),
            &1,
            &0,
            &1,
        );
    }

    #[test]
    fn writes_extend_the_storage_lifetime() {
        let (env, client, owner) = setup();
        let license = hash(&env, 1);
        client.record(
            &owner,
            &license,
            &1,
            &hash(&env, 10),
            &zero(&env),
            &1,
            &0,
            &1,
        );
        let id = client.address.clone();
        let ttl = env.as_contract(&id, || {
            env.storage()
                .persistent()
                .get_ttl(&DataKey::Receipt(owner.clone(), license.clone(), 1))
        });
        assert_eq!(ttl, TTL_EXTEND_TO);
        assert_eq!(
            env.as_contract(&id, || env.storage().instance().get_ttl()),
            TTL_EXTEND_TO
        );
    }
}
