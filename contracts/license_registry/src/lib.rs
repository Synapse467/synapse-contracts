#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    LicenseAlreadyExists = 2,
    LicenseNotFound = 3,
    LicenseAlreadyRevoked = 4,
    InvalidValidityPeriod = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseRecord {
    pub licensor: Address,
    pub capsule_version_ref: BytesN<32>,
    pub grantee: Address,
    pub terms_hash: BytesN<32>,
    pub starts_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
}

#[contracttype]
pub enum DataKey {
    License(BytesN<32>),
}

#[contract]
pub struct LicenseRegistry;

#[contractimpl]
impl LicenseRegistry {
    pub fn grant(
        env: Env,
        license_ref: BytesN<32>,
        licensor: Address,
        capsule_version_ref: BytesN<32>,
        grantee: Address,
        terms_hash: BytesN<32>,
        starts_at: u64,
        expires_at: u64,
    ) -> Result<(), Error> {
        licensor.require_auth();

        if expires_at <= starts_at {
            return Err(Error::InvalidValidityPeriod);
        }

        let key = DataKey::License(license_ref);
        if env.storage().persistent().has(&key) {
            return Err(Error::LicenseAlreadyExists);
        }

        let record = LicenseRecord {
            licensor,
            capsule_version_ref,
            grantee,
            terms_hash,
            starts_at,
            expires_at,
            revoked: false,
        };

        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn revoke(env: Env, license_ref: BytesN<32>) -> Result<(), Error> {
        let key = DataKey::License(license_ref);
        let mut record: LicenseRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::LicenseNotFound)?;

        record.licensor.require_auth();

        if record.revoked {
            return Err(Error::LicenseAlreadyRevoked);
        }

        record.revoked = true;
        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    pub fn is_active(env: Env, license_ref: BytesN<32>, now: u64) -> bool {
        let key = DataKey::License(license_ref);
        if let Some(record) = env
            .storage()
            .persistent()
            .get::<DataKey, LicenseRecord>(&key)
        {
            !record.revoked && now >= record.starts_at && now <= record.expires_at
        } else {
            false
        }
    }

    pub fn get_license(env: Env, license_ref: BytesN<32>) -> Option<LicenseRecord> {
        let key = DataKey::License(license_ref);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_license_grant_active_and_revocation() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(LicenseRegistry, ());
        let client = LicenseRegistryClient::new(&env, &contract_id);

        let licensor = Address::generate(&env);
        let grantee = Address::generate(&env);
        let license_ref = BytesN::from_array(&env, &[1u8; 32]);
        let version_ref = BytesN::from_array(&env, &[2u8; 32]);
        let terms_hash = BytesN::from_array(&env, &[3u8; 32]);

        let starts_at = 1000;
        let expires_at = 5000;

        client.grant(
            &license_ref,
            &licensor,
            &version_ref,
            &grantee,
            &terms_hash,
            &starts_at,
            &expires_at,
        );

        // Active within window
        assert!(client.is_active(&license_ref, &2000));
        // Inactive before start
        assert!(!client.is_active(&license_ref, &500));
        // Inactive after expiration
        assert!(!client.is_active(&license_ref, &6000));

        // Revoke license
        client.revoke(&license_ref);
        // Now inactive even within former validity window
        assert!(!client.is_active(&license_ref, &2000));
    }

    #[test]
    #[should_panic]
    fn test_unauthorized_revoke_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(LicenseRegistry, ());
        let client = LicenseRegistryClient::new(&env, &contract_id);

        let licensor = Address::generate(&env);
        let grantee = Address::generate(&env);
        let license_ref = BytesN::from_array(&env, &[50u8; 32]);
        let version_ref = BytesN::from_array(&env, &[51u8; 32]);
        let terms_hash = BytesN::from_array(&env, &[52u8; 32]);

        client.grant(
            &license_ref,
            &licensor,
            &version_ref,
            &grantee,
            &terms_hash,
            &1000,
            &5000,
        );

        // No signature/mock is provided for the licensor-gated revoke below,
        // so `record.licensor.require_auth()` must reject it — a grantee or
        // third party must never be able to revoke someone else's license.
        env.set_auths(&[]);
        client.revoke(&license_ref);
    }
}
