use cw_storage_plus::{Item, Strategy, Map, SnapshotMap, SnapshotItem};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Timestamp, Addr, Uint128, BlockInfo, StdResult, StdError};

pub const ACCOUNTS: Map<&Addr, Account> = Map::new(
    "vesting_account",
);

pub const VESTING_DATA: SnapshotMap<&Addr, VestingData> = SnapshotMap::new(
    "vesting_data",
    "vesting_data__checkpoints",
    "vesting_data__changelog",
    Strategy::EveryBlock,
);

pub const VESTING_TOTAL: SnapshotItem<TotalVestingInfo> = SnapshotItem::new(
    "vesting_total",
    "vesting_total__checkpoints",
    "vesting_total__changelog",
    Strategy::EveryBlock,
);
pub const TOKEN_ADDRESS: Item<Addr> = Item::new("token_address");
pub const OWNER_ADDRESS: Item<Addr> = Item::new("owner_address");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct TotalVestingInfo {
    pub prevesting_amount: Uint128,
    pub prevested_amount: Uint128,
    pub vesting_amount: Uint128,
    pub vested_amount: Uint128,
    pub claimed_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Account {
    pub address: Addr,
    pub prevesting_amount: Uint128,
    pub vesting_amount: Uint128,
    pub claimed_amount: Uint128,
    pub registration_time: Timestamp,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, PartialEq, Debug)]
pub struct VestingData {
    pub prevesting_amount: Uint128,
    pub prevested_amount: Uint128,
    pub vesting_amount: Uint128,
    pub vested_amount: Uint128,
    pub claimable_amount: Uint128,
    pub claimed_amount: Uint128,
    pub registration_time: Timestamp,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
}

impl Default for VestingData {
    fn default() -> Self {
        VestingData {
            prevesting_amount: Uint128::zero(),
            prevested_amount: Uint128::zero(),
            vesting_amount: Uint128::zero(),
            vested_amount: Uint128::zero(),
            claimable_amount: Uint128::zero(),
            claimed_amount: Uint128::zero(),
            registration_time: Timestamp::default(),
            start_time: Timestamp::default(),
            end_time: Timestamp::default(),
        }
    }
}

impl Account {
    pub fn validate(&self, block_info: &BlockInfo) -> StdResult<()> {

        if self.prevesting_amount.is_zero() {
            return Err(StdError::generic_err("assert(prevesting_amount > 0)"));
        }

        if self.vesting_amount.is_zero() {
            return Err(StdError::generic_err("assert(vesting_amount > 0)"));
        }

        if self.registration_time < block_info.time {
            return Err(StdError::generic_err("assert(registration_time >= block_time)"));
        }

        if self.start_time < self.registration_time {
            return Err(StdError::generic_err("assert(start_time >= registration_time)"));
        }

        if self.start_time < block_info.time {
            return Err(StdError::generic_err("assert(start_time >= block_time)"));
        }

        if self.end_time < self.start_time {
            return Err(StdError::generic_err("assert(end_time >= start_time)"));
        }

        Ok(())
    }

    pub fn vested_amount(&self, block_info: &BlockInfo) -> StdResult<Uint128> {
        if block_info.time < self.start_time {
            return Ok(Uint128::zero());
        }

        if block_info.time >= self.end_time {
            return Ok(self.vesting_amount.clone());
        }

        let vested_token = self.vesting_amount
                    .checked_mul(Uint128::from(block_info.time.nanos() - self.start_time.nanos()))?
                    .checked_div(Uint128::from(self.end_time.nanos() - self.start_time.nanos()))?;

        Ok(vested_token)
    }

    pub fn prevested_amount(&self, block_info: &BlockInfo) -> StdResult<Uint128> {

        if block_info.time < self.registration_time {
            return Ok(Uint128::zero());
        }

        if block_info.time >= self.start_time {
            return Ok(self.vesting_amount.clone());
        }

        let prevested_token = self.vesting_amount
                    .checked_mul(Uint128::from(block_info.time.nanos() - self.registration_time.nanos()))?
                    .checked_div(Uint128::from(self.start_time.nanos() - self.registration_time.nanos()))?;
        
        if self.prevesting_amount > prevested_token {
            return Ok(self.prevesting_amount.clone());
        }

        Ok(prevested_token)
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{Addr, Uint128, Timestamp, testing::mock_env};

    use super::Account;

    #[test]
    fn test_vested_amount() {
        let account = Account {
            address: Addr::unchecked("addr00001".to_string()),
            prevesting_amount: Uint128::new(10u128),
            vesting_amount: Uint128::new(100u128),
            claimed_amount: Uint128::zero(),
            registration_time: Timestamp::from_nanos(0),
            start_time: Timestamp::from_nanos(50),
            end_time: Timestamp::from_nanos(100),
        };

        // Check fixed vesting before start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(20);

        let vested_tokens = account.vested_amount(&env.block).unwrap();
        let prevested_tokens = account.prevested_amount(&env.block).unwrap();
        assert_eq!(vested_tokens, Uint128::new(0));
        assert_eq!(prevested_tokens, Uint128::new(40));

        // Check fixed vesting before start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(40);

        let vested_tokens = account.vested_amount(&env.block).unwrap();
        let prevested_tokens = account.prevested_amount(&env.block).unwrap();
        assert_eq!(vested_tokens, Uint128::new(0));
        assert_eq!(prevested_tokens, Uint128::new(80));

        // Check vesting after start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(70);

        let vested_tokens = account.vested_amount(&env.block).unwrap();
        let prevested_tokens = account.prevested_amount(&env.block).unwrap();
        assert_eq!(vested_tokens, Uint128::new(40));
        assert_eq!(prevested_tokens, Uint128::new(100));

        // Check vesting after end time
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(102);

        let vested_tokens = account.vested_amount(&env.block).unwrap();
        let prevested_tokens = account.prevested_amount(&env.block).unwrap();
        assert_eq!(vested_tokens, Uint128::new(100));
        assert_eq!(prevested_tokens, Uint128::new(100));
    }

}

pub fn get_vesting_data_from_account(account: Account, block_info: &BlockInfo) -> StdResult<VestingData> {
    let vested_amount = account.vested_amount(&block_info)?;
    let prevested_amount = account.prevested_amount(&block_info)?;
    let claimed_amount = account.claimed_amount;

    let claimable_amount = vested_amount.checked_sub(claimed_amount)?;

    let vesting_data = VestingData {
        prevesting_amount: account.prevesting_amount,
        prevested_amount,
        vesting_amount: account.vesting_amount,
        vested_amount,
        registration_time: account.registration_time,
        start_time: account.start_time,
        end_time: account.end_time,
        claimable_amount: claimable_amount,
        claimed_amount: account.claimed_amount,
    };

    Ok(vesting_data)
}
