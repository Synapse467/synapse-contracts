#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ExpertAlreadyRegistered = 4,
    ExpertNotFound = 5,
    InvalidStatus = 6,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerificationStatus {
    Unverified = 0,
    Pending = 1,
    Verified = 2,
    Revoked = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpertRecord {
    pub controller: Address,
    pub metadata_hash: BytesN<32>,
    pub status: u32,
    pub registered_at: u64,
    pub updated_at: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Expert(BytesN<32>),
}

#[contract]
pub struct ExpertRegistry;

#[contractimpl]
impl ExpertRegistry {
    pub fn init(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn register_expert(
        env: Env,
        expert_ref: BytesN<32>,
        controller: Address,
        metadata_hash: BytesN<32>,
    ) -> Result<(), Error> {
        controller.require_auth();
        let key = DataKey::Expert(expert_ref.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::ExpertAlreadyRegistered);
        }

        let now = env.ledger().timestamp();
        let record = ExpertRecord {
            controller,
            metadata_hash,
            status: VerificationStatus::Unverified as u32,
            registered_at: now,
            updated_at: now,
        };

        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn update_verification(
        env: Env,
        expert_ref: BytesN<32>,
        new_status: u32,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        if new_status > (VerificationStatus::Revoked as u32) {
            return Err(Error::InvalidStatus);
        }

        let key = DataKey::Expert(expert_ref);
        let mut record: ExpertRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ExpertNotFound)?;

        record.status = new_status;
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn get_expert(env: Env, expert_ref: BytesN<32>) -> Option<ExpertRecord> {
        let key = DataKey::Expert(expert_ref);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_expert_registration_and_verification() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(ExpertRegistry, ());
        let client = ExpertRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let expert_ref = BytesN::from_array(&env, &[1u8; 32]);
        let controller = Address::generate(&env);
        let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);

        client.register_expert(&expert_ref, &controller, &metadata_hash);

        let record = client.get_expert(&expert_ref).unwrap();
        assert_eq!(record.controller, controller);
        assert_eq!(record.metadata_hash, metadata_hash);
        assert_eq!(record.status, VerificationStatus::Unverified as u32);

        // Verify status update
        client.update_verification(&expert_ref, &(VerificationStatus::Verified as u32));
        let updated = client.get_expert(&expert_ref).unwrap();
        assert_eq!(updated.status, VerificationStatus::Verified as u32);

        // Duplicate registration fails
        let res = client.try_register_expert(&expert_ref, &controller, &metadata_hash);
        assert_eq!(res, Err(Ok(Error::ExpertAlreadyRegistered)));
    }
}
