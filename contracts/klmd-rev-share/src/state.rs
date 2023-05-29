use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_storage_plus::{Item, Map};
use cw_utils::{Expiration, Scheduled};

#[cw_serde]
pub struct Config {
    /// Owner If None set, contract is frozen.
    pub owner: Option<Addr>,
    pub cw20_token_address: Addr,
    pub native_token: String,
    pub cw20_staking_address: Addr,
}

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const LATEST_STAGE_KEY: &str = "stage";
pub const LATEST_STAGE: Item<u8> = Item::new(LATEST_STAGE_KEY);

pub const STAGE_EXPIRATION_KEY: &str = "stage_exp";
pub const STAGE_EXPIRATION: Map<u8, Expiration> = Map::new(STAGE_EXPIRATION_KEY);

pub const STAGE_START_KEY: &str = "stage_start";
pub const STAGE_START: Map<u8, Scheduled> = Map::new(STAGE_START_KEY);

pub const STAGE_AMOUNT_KEY: &str = "stage_amount";
pub const STAGE_AMOUNT: Map<u8, Uint128> = Map::new(STAGE_AMOUNT_KEY);

pub const CLAIM_PREFIX: &str = "claim";
pub const CLAIM: Map<(String, u8), bool> = Map::new(CLAIM_PREFIX);

pub const STAGE_AMOUNT_CLAIMED_KEY: &str = "stage_claimed_amount";
pub const STAGE_AMOUNT_CLAIMED: Map<u8, Uint128> = Map::new(STAGE_AMOUNT_CLAIMED_KEY);

pub const STAGE_PAUSED_KEY: &str = "stage_paused";
pub const STAGE_PAUSED: Map<u8, bool> = Map::new(STAGE_PAUSED_KEY);

pub const STAGE_HEIGHT_KEY: &str = "stage_height";
pub const STAGE_HEIGHT: Map<u8, Uint64> = Map::new(STAGE_HEIGHT_KEY);

pub fn compute_allocation(
    stage_amount: Uint128,
    total_staked: Uint128,
    account_stake: Uint128,
) -> Uint128 {
    account_stake.checked_multiply_ratio(
        stage_amount, total_staked).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Uint128;

    #[test]
    fn compute_allocation() {
        let stage_amount = Uint128::new(1000);
        let total_staked = Uint128::new(10000);
        let account_stake = Uint128::new(1000);
        let allocation = super::compute_allocation(stage_amount, total_staked, account_stake);
        assert_eq!(allocation, Uint128::new(100));

        let stage_amount = Uint128::new(1000);
        let total_staked = Uint128::new(10000);
        let account_stake = Uint128::new(0);

        let allocation = super::compute_allocation(stage_amount, total_staked, account_stake);
        assert_eq!(allocation, Uint128::new(0));
    }
}
