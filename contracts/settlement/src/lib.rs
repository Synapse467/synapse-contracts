#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InvalidAmount = 1,
    EmptyContributors = 2,
    InvalidSharesSum = 3,
    SettlementAlreadyExists = 4,
    SettlementNotFound = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContributorShare {
    pub recipient: Address,
    pub share_bps: u32, // basis points, e.g. 5000 = 50%
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutEntry {
    pub recipient: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementRecord {
    pub total_amount: i128,
    pub payouts: Vec<PayoutEntry>,
    pub settled_at: u64,
}

#[contracttype]
pub enum DataKey {
    Settlement(BytesN<32>),
}

#[contract]
pub struct Settlement;

#[contractimpl]
impl Settlement {
    pub fn settle_split(
        env: Env,
        settlement_ref: BytesN<32>,
        payer: Address,
        total_amount: i128,
        shares: Vec<ContributorShare>,
    ) -> Result<Vec<PayoutEntry>, Error> {
        payer.require_auth();

        if total_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if shares.is_empty() {
            return Err(Error::EmptyContributors);
        }

        let key = DataKey::Settlement(settlement_ref);
        if env.storage().persistent().has(&key) {
            return Err(Error::SettlementAlreadyExists);
        }

        let mut total_bps: u32 = 0;
        for share in shares.iter() {
            total_bps = total_bps.checked_add(share.share_bps).ok_or(Error::InvalidSharesSum)?;
        }
        if total_bps != 10_000 {
            return Err(Error::InvalidSharesSum);
        }

        let mut payouts: Vec<PayoutEntry> = Vec::new(&env);
        let mut distributed: i128 = 0;
        let len = shares.len();

        for (idx, share) in shares.iter().enumerate() {
            let amount = if idx == (len - 1) as usize {
                // Ensure no rounding remainder is lost
                total_amount - distributed
            } else {
                (total_amount * (share.share_bps as i128)) / 10_000
            };

            distributed += amount;
            payouts.push_back(PayoutEntry {
                recipient: share.recipient,
                amount,
            });
        }

        let record = SettlementRecord {
            total_amount,
            payouts: payouts.clone(),
            settled_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &record);
        Ok(payouts)
    }

    pub fn get_settlement(env: Env, settlement_ref: BytesN<32>) -> Option<SettlementRecord> {
        let key = DataKey::Settlement(settlement_ref);
        env.storage().persistent().get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{vec, Env};

    #[test]
    fn test_settlement_split_arithmetic_and_remainder_handling() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Settlement, ());
        let client = SettlementClient::new(&env, &contract_id);

        let payer = Address::generate(&env);
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);
        let c3 = Address::generate(&env);

        let settlement_ref = BytesN::from_array(&env, &[77u8; 32]);
        let total_amount: i128 = 100_001; // Odd amount testing remainder conservation

        // 33.33%, 33.33%, 33.34% = 10000 bps
        let shares = vec![
            &env,
            ContributorShare {
                recipient: c1.clone(),
                share_bps: 3333,
            },
            ContributorShare {
                recipient: c2.clone(),
                share_bps: 3333,
            },
            ContributorShare {
                recipient: c3.clone(),
                share_bps: 3334,
            },
        ];

        let payouts = client.settle_split(&settlement_ref, &payer, &total_amount, &shares);

        assert_eq!(payouts.len(), 3);
        let p1 = payouts.get(0).unwrap().amount;
        let p2 = payouts.get(1).unwrap().amount;
        let p3 = payouts.get(2).unwrap().amount;

        // Exactly sums to total without rounding loss
        assert_eq!(p1 + p2 + p3, total_amount);

        let record = client.get_settlement(&settlement_ref).unwrap();
        assert_eq!(record.total_amount, total_amount);
    }
}
