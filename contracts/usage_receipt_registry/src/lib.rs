#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ReceiptAlreadyExists = 4,
    ReceiptNotFound = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageReceiptRecord {
    pub license_ref: BytesN<32>,
    pub usage_manifest_hash: BytesN<32>,
    pub period: u64,
    pub recorded_at: u64,
    pub recorder: Address,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Receipt(BytesN<32>),
}

#[contract]
pub struct UsageReceiptRegistry;

#[contractimpl]
impl UsageReceiptRegistry {
    pub fn init(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn record(
        env: Env,
        receipt_ref: BytesN<32>,
        license_ref: BytesN<32>,
        usage_manifest_hash: BytesN<32>,
        period: u64,
        recorder: Address,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;

        // Must be authorized by admin or registered recorder
        if recorder != admin {
            return Err(Error::Unauthorized);
        }
        recorder.require_auth();

        let key = DataKey::Receipt(receipt_ref);
        if env.storage().persistent().has(&key) {
            return Err(Error::ReceiptAlreadyExists);
        }

        let record = UsageReceiptRecord {
            license_ref,
            usage_manifest_hash,
            period,
            recorded_at: env.ledger().timestamp(),
            recorder,
        };

        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn get_receipt(env: Env, receipt_ref: BytesN<32>) -> Option<UsageReceiptRecord> {
        let key = DataKey::Receipt(receipt_ref);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_usage_receipt_recording_and_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(UsageReceiptRegistry, ());
        let client = UsageReceiptRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let receipt_ref = BytesN::from_array(&env, &[1u8; 32]);
        let license_ref = BytesN::from_array(&env, &[2u8; 32]);
        let usage_manifest_hash = BytesN::from_array(&env, &[3u8; 32]);
        let period = 20260901;

        client.record(
            &receipt_ref,
            &license_ref,
            &usage_manifest_hash,
            &period,
            &admin,
        );

        let receipt = client.get_receipt(&receipt_ref).unwrap();
        assert_eq!(receipt.license_ref, license_ref);
        assert_eq!(receipt.usage_manifest_hash, usage_manifest_hash);
        assert_eq!(receipt.period, period);

        // Unauthorized recorder rejected
        let rando = Address::generate(&env);
        let receipt_ref2 = BytesN::from_array(&env, &[4u8; 32]);
        let res = client.try_record(
            &receipt_ref2,
            &license_ref,
            &usage_manifest_hash,
            &period,
            &rando,
        );
        assert_eq!(res, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    #[should_panic]
    fn test_record_without_signature_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(UsageReceiptRegistry, ());
        let client = UsageReceiptRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        // Even though `recorder == admin` passes the business-logic check,
        // no signature/mock is provided here, so `recorder.require_auth()`
        // must still reject the call — the identity check alone is not
        // sufficient authorization.
        env.set_auths(&[]);
        let receipt_ref = BytesN::from_array(&env, &[9u8; 32]);
        let license_ref = BytesN::from_array(&env, &[10u8; 32]);
        let usage_manifest_hash = BytesN::from_array(&env, &[11u8; 32]);
        client.record(&receipt_ref, &license_ref, &usage_manifest_hash, &1, &admin);
    }
}
