#![no_std]
//! A public record of expertise licenses: who granted what to whom, until when, and whether it
//! has been revoked.
//!
//! The full license (purposes, caps, terms) is a signed file shared off-chain. Only its hash goes
//! on-chain, so terms stay private but anyone holding the file can prove it is the one recorded.
//! There is no admin: each record lives under its grantor, and only the grantor can revoke it.
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
    AlreadyExists = 1,
    NotFound = 2,
    AlreadyRevoked = 3,
    InvalidExpiry = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct License {
    pub capsule_ref: BytesN<32>,
    pub grantee: Address,
    pub terms_hash: BytesN<32>,
    pub granted_at: u64,
    // Ledger time after which the license no longer applies. 0 means it does not expire.
    pub expires_at: u64,
    pub revoked: bool,
    pub revoked_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    License(Address, BytesN<32>),
}

#[contractevent(topics = ["license_granted"])]
pub struct LicenseGranted {
    #[topic]
    pub grantor: Address,
    #[topic]
    pub license_ref: BytesN<32>,
    pub grantee: Address,
}

#[contractevent(topics = ["license_revoked"])]
pub struct LicenseRevoked {
    #[topic]
    pub grantor: Address,
    #[topic]
    pub license_ref: BytesN<32>,
}

#[contract]
pub struct LicenseLedger;

fn renew(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

#[contractimpl]
impl LicenseLedger {
    pub fn grant(
        env: Env,
        grantor: Address,
        license_ref: BytesN<32>,
        capsule_ref: BytesN<32>,
        grantee: Address,
        terms_hash: BytesN<32>,
        expires_at: u64,
    ) -> Result<(), Error> {
        grantor.require_auth();
        let now = env.ledger().timestamp();
        if expires_at != 0 && expires_at <= now {
            return Err(Error::InvalidExpiry);
        }
        let key = DataKey::License(grantor.clone(), license_ref.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }
        env.storage().persistent().set(
            &key,
            &License {
                capsule_ref,
                grantee: grantee.clone(),
                terms_hash,
                granted_at: now,
                expires_at,
                revoked: false,
                revoked_at: 0,
            },
        );
        renew(&env, &key);
        LicenseGranted {
            grantor,
            license_ref,
            grantee,
        }
        .publish(&env);
        Ok(())
    }

    // Only the grantor that wrote a license can revoke it: the record is looked up under the
    // authorizing address, so nobody else can even address it.
    pub fn revoke(env: Env, grantor: Address, license_ref: BytesN<32>) -> Result<(), Error> {
        grantor.require_auth();
        let key = DataKey::License(grantor.clone(), license_ref.clone());
        let mut license: License = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;
        if license.revoked {
            return Err(Error::AlreadyRevoked);
        }
        license.revoked = true;
        license.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &license);
        renew(&env, &key);
        LicenseRevoked {
            grantor,
            license_ref,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get(env: Env, grantor: Address, license_ref: BytesN<32>) -> Option<License> {
        env.storage()
            .persistent()
            .get(&DataKey::License(grantor, license_ref))
    }

    pub fn is_active(env: Env, grantor: Address, license_ref: BytesN<32>) -> bool {
        let now = env.ledger().timestamp();
        Self::get(env, grantor, license_ref)
            .map(|l| !l.revoked && (l.expires_at == 0 || now < l.expires_at))
            .unwrap_or(false)
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
    fn setup() -> (Env, LicenseLedgerClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        let id = env.register(LicenseLedger, ());
        let client = LicenseLedgerClient::new(&env, &id);
        let grantor = Address::generate(&env);
        let grantee = Address::generate(&env);
        (env, client, grantor, grantee)
    }

    #[test]
    fn a_granted_license_is_active_until_it_expires() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &5_000,
        );
        assert!(client.is_active(&grantor, &license));
        let record = client.get(&grantor, &license).unwrap();
        assert_eq!(record.grantee, grantee);
        assert_eq!(record.terms_hash, hash(&env, 3));
        assert_eq!(record.granted_at, 1_000);
        env.ledger().with_mut(|l| l.timestamp = 4_999);
        assert!(client.is_active(&grantor, &license));
        env.ledger().with_mut(|l| l.timestamp = 5_000);
        assert!(!client.is_active(&grantor, &license));
    }

    #[test]
    fn a_license_with_no_expiry_stays_active() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        env.ledger().with_mut(|l| l.timestamp = u64::MAX);
        assert!(client.is_active(&grantor, &license));
    }

    #[test]
    fn revocation_is_immediate_and_permanent() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        env.ledger().with_mut(|l| l.timestamp = 2_000);
        client.revoke(&grantor, &license);
        assert!(!client.is_active(&grantor, &license));
        let record = client.get(&grantor, &license).unwrap();
        assert!(record.revoked);
        assert_eq!(record.revoked_at, 2_000);
        assert_eq!(
            client.try_revoke(&grantor, &license),
            Err(Ok(Error::AlreadyRevoked))
        );
    }

    #[test]
    fn rejects_duplicates_past_expiries_and_unknown_licenses() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        assert_eq!(
            client.try_grant(
                &grantor,
                &license,
                &hash(&env, 2),
                &grantee,
                &hash(&env, 3),
                &1_000
            ),
            Err(Ok(Error::InvalidExpiry))
        );
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        assert_eq!(
            client.try_grant(
                &grantor,
                &license,
                &hash(&env, 9),
                &grantee,
                &hash(&env, 9),
                &0
            ),
            Err(Ok(Error::AlreadyExists))
        );
        assert_eq!(
            client.try_revoke(&grantor, &hash(&env, 77)),
            Err(Ok(Error::NotFound))
        );
        // The first grant is untouched by the rejected duplicate.
        assert_eq!(
            client.get(&grantor, &license).unwrap().terms_hash,
            hash(&env, 3)
        );
    }

    #[test]
    fn only_the_grantor_can_revoke() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        // The grantee, or anyone else, addresses their own namespace and finds nothing.
        assert_eq!(
            client.try_revoke(&grantee, &license),
            Err(Ok(Error::NotFound))
        );
        let stranger = Address::generate(&env);
        assert_eq!(
            client.try_revoke(&stranger, &license),
            Err(Ok(Error::NotFound))
        );
        assert!(client.is_active(&grantor, &license));
    }

    #[test]
    fn licenses_are_namespaced_by_grantor() {
        let (env, client, alice, grantee) = setup();
        let bob = Address::generate(&env);
        let license = hash(&env, 1);
        client.grant(
            &alice,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        // Bob may reuse the same reference: nobody can squat another grantor's reference.
        client.grant(&bob, &license, &hash(&env, 4), &grantee, &hash(&env, 5), &0);
        client.revoke(&bob, &license);
        assert!(client.is_active(&alice, &license));
        assert!(!client.is_active(&bob, &license));
    }

    #[test]
    fn emits_events_for_grant_and_revoke() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        let id = client.address.clone();
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        assert_eq!(
            env.events().all(),
            std::vec![LicenseGranted {
                grantor: grantor.clone(),
                license_ref: license.clone(),
                grantee: grantee.clone()
            }
            .to_xdr(&env, &id)]
        );
        client.revoke(&grantor, &license);
        assert_eq!(
            env.events().all(),
            std::vec![LicenseRevoked {
                grantor: grantor.clone(),
                license_ref: license.clone()
            }
            .to_xdr(&env, &id)]
        );
    }

    #[test]
    #[should_panic]
    fn an_unauthorized_grant_is_rejected() {
        let env = Env::default();
        let id = env.register(LicenseLedger, ());
        LicenseLedgerClient::new(&env, &id).grant(
            &Address::generate(&env),
            &hash(&env, 1),
            &hash(&env, 2),
            &Address::generate(&env),
            &hash(&env, 3),
            &0,
        );
    }

    #[test]
    fn unknown_licenses_are_not_active() {
        let (env, client, grantor, _) = setup();
        assert!(!client.is_active(&grantor, &hash(&env, 42)));
        assert!(client.get(&grantor, &hash(&env, 42)).is_none());
    }

    #[test]
    fn writes_extend_the_storage_lifetime() {
        let (env, client, grantor, grantee) = setup();
        let license = hash(&env, 1);
        client.grant(
            &grantor,
            &license,
            &hash(&env, 2),
            &grantee,
            &hash(&env, 3),
            &0,
        );
        let id = client.address.clone();
        let entry = || {
            env.as_contract(&id, || {
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::License(grantor.clone(), license.clone()))
            })
        };
        assert_eq!(entry(), TTL_EXTEND_TO);
        assert_eq!(
            env.as_contract(&id, || env.storage().instance().get_ttl()),
            TTL_EXTEND_TO
        );
        client.revoke(&grantor, &license);
        assert!(entry() >= TTL_THRESHOLD);
    }
}
