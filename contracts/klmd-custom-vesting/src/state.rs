use cw_storage_plus::{Map, Item};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Timestamp, Addr, Uint128, BlockInfo, StdResult, StdError};

pub const ACCOUNTS: Map<&Addr, Account> = Map::new("accounts");
pub const TOKEN_ADDRESS: Item<Addr> = Item::new("token_address");
pub const OWNER_ADDRESS: Item<Addr> = Item::new("owner_address");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Account {
    pub address: Addr,
    pub vesting_amount: Uint128,
    pub claimed_amount: Uint128,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
}

impl Account {
    pub fn validate(&self, block_info: &BlockInfo) -> StdResult<()> {

        if self.vesting_amount.is_zero() {
            return Err(StdError::generic_err("assert(vesting_amount > 0)"));
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
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{Addr, Uint128, Timestamp, testing::mock_env};

    use super::Account;

    #[test]
    fn test_vested_amount() {
        let account = Account {
            address: Addr::unchecked("addr00001".to_string()),
            vesting_amount: Uint128::new(100u128),
            claimed_amount: Uint128::zero(),
            start_time: Timestamp::from_nanos(0),
            end_time: Timestamp::from_nanos(100),
        };
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(50);
        
        let vested_tokens = account.vested_amount(&env.block).unwrap();

        assert_eq!(vested_tokens, Uint128::new(50));

        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(0);
        
        let vested_tokens = account.vested_amount(&env.block).unwrap();

        assert_eq!(vested_tokens, Uint128::new(0));


        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(102);
        
        let vested_tokens = account.vested_amount(&env.block).unwrap();

        assert_eq!(vested_tokens, Uint128::new(100));
    }

}