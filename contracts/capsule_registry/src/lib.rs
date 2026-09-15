#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    CapsuleAlreadyExists = 2,
    CapsuleNotFound = 3,
    VersionAlreadyPublished = 4,
    VersionNotFound = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapsuleRecord {
    pub owner: Address,
    pub metadata_hash: BytesN<32>,
    pub registered_at: u64,
    pub latest_version: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionRecord {
    pub manifest_hash: BytesN<32>,
    pub eval_hash: BytesN<32>,
    pub published_at: u64,
}

#[contracttype]
pub enum DataKey {
    Capsule(BytesN<32>),
    Version(BytesN<32>, BytesN<32>), // (capsule_ref, version_ref)
}

#[contract]
pub struct CapsuleRegistry;

#[contractimpl]
impl CapsuleRegistry {
    pub fn register_capsule(
        env: Env,
        capsule_ref: BytesN<32>,
        owner: Address,
        metadata_hash: BytesN<32>,
    ) -> Result<(), Error> {
        owner.require_auth();
        let key = DataKey::Capsule(capsule_ref);
        if env.storage().persistent().has(&key) {
            return Err(Error::CapsuleAlreadyExists);
        }

        let record = CapsuleRecord {
            owner,
            metadata_hash,
            registered_at: env.ledger().timestamp(),
            latest_version: BytesN::from_array(&env, &[0u8; 32]),
        };

        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn publish_version(
        env: Env,
        capsule_ref: BytesN<32>,
        version_ref: BytesN<32>,
        manifest_hash: BytesN<32>,
        eval_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let capsule_key = DataKey::Capsule(capsule_ref.clone());
        let mut capsule: CapsuleRecord = env
            .storage()
            .persistent()
            .get(&capsule_key)
            .ok_or(Error::CapsuleNotFound)?;

        capsule.owner.require_auth();

        let version_key = DataKey::Version(capsule_ref, version_ref.clone());
        if env.storage().persistent().has(&version_key) {
            // Invariant: published version entries are immutable
            return Err(Error::VersionAlreadyPublished);
        }

        let version_record = VersionRecord {
            manifest_hash,
            eval_hash,
            published_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&version_key, &version_record);

        capsule.latest_version = version_ref;
        env.storage().persistent().set(&capsule_key, &capsule);
        Ok(())
    }

    pub fn get_capsule(env: Env, capsule_ref: BytesN<32>) -> Option<CapsuleRecord> {
        let key = DataKey::Capsule(capsule_ref);
        env.storage().persistent().get(&key)
    }

    pub fn get_version(
        env: Env,
        capsule_ref: BytesN<32>,
        version_ref: BytesN<32>,
    ) -> Option<VersionRecord> {
        let key = DataKey::Version(capsule_ref, version_ref);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_capsule_registration_and_immutable_version_publishing() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CapsuleRegistry, ());
        let client = CapsuleRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let capsule_ref = BytesN::from_array(&env, &[10u8; 32]);
        let metadata_hash = BytesN::from_array(&env, &[11u8; 32]);

        client.register_capsule(&capsule_ref, &owner, &metadata_hash);
        let capsule = client.get_capsule(&capsule_ref).unwrap();
        assert_eq!(capsule.owner, owner);

        let version_ref = BytesN::from_array(&env, &[1u8; 32]);
        let manifest_hash = BytesN::from_array(&env, &[20u8; 32]);
        let eval_hash = BytesN::from_array(&env, &[30u8; 32]);

        client.publish_version(&capsule_ref, &version_ref, &manifest_hash, &eval_hash);

        let ver = client.get_version(&capsule_ref, &version_ref).unwrap();
        assert_eq!(ver.manifest_hash, manifest_hash);
        assert_eq!(ver.eval_hash, eval_hash);

        // Attempting to republish the same version must fail (immutability guarantee)
        let dup_eval = BytesN::from_array(&env, &[99u8; 32]);
        let res = client.try_publish_version(&capsule_ref, &version_ref, &manifest_hash, &dup_eval);
        assert_eq!(res, Err(Ok(Error::VersionAlreadyPublished)));
    }

    #[test]
    #[should_panic]
    fn test_unauthorized_version_publish_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CapsuleRegistry, ());
        let client = CapsuleRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let capsule_ref = BytesN::from_array(&env, &[40u8; 32]);
        let metadata_hash = BytesN::from_array(&env, &[41u8; 32]);
        client.register_capsule(&capsule_ref, &owner, &metadata_hash);

        // No signature/mock is provided for the owner-gated call below, so
        // `capsule.owner.require_auth()` must reject it, preserving
        // immutability/ownership even against a caller who supplies no auth.
        env.set_auths(&[]);
        let version_ref = BytesN::from_array(&env, &[42u8; 32]);
        let manifest_hash = BytesN::from_array(&env, &[43u8; 32]);
        let eval_hash = BytesN::from_array(&env, &[44u8; 32]);
        client.publish_version(&capsule_ref, &version_ref, &manifest_hash, &eval_hash);
    }
}
