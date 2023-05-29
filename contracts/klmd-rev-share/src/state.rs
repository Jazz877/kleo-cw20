use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
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

pub const STAGE_DISTRIBUTIONS_KEY: &str = "stage_distributions";
pub const STAGE_DISTRIBUTIONS: Map<(u8, Addr), Uint128> = Map::new(STAGE_DISTRIBUTIONS_KEY);

pub fn compute_allocations(
    total_amount: Uint128,
    staker_snapshot: Vec<(Addr, Uint128)>,
) -> Vec<(Addr, Uint128)> {
    let mut allocations: Vec<(Addr, Uint128)> = Vec::new();
    let total_staked: Uint128 = staker_snapshot.iter().fold(Uint128::zero(), |acc, (_, stake)| acc + stake);
    for (addr, cw20_stake) in staker_snapshot {
        let allocation = cw20_stake.checked_multiply_ratio(
            total_amount, total_staked).unwrap_or_default();
        allocations.push((addr, Uint128::from(allocation)));
    }
    allocations
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Uint128};

    #[test]
    fn compute_allocations() {
        let total_amount = Uint128::from(100_000u32);
        let staker_snapshot = vec![
            (Addr::unchecked("addr1"), Uint128::from(10u32)),
            (Addr::unchecked("addr2"), Uint128::from(20u32)),
            (Addr::unchecked("addr3"), Uint128::from(30u32)),
            (Addr::unchecked("addr4"), Uint128::from(40u32)),
        ];

        let allocations = super::compute_allocations(total_amount, staker_snapshot.clone());
        let mut expected_allocations: Vec<(Addr, Uint128)> = vec![
            (Addr::unchecked("addr1"), Uint128::from(10_000u32)),
            (Addr::unchecked("addr2"), Uint128::from(20_000u32)),
            (Addr::unchecked("addr3"), Uint128::from(30_000u32)),
            (Addr::unchecked("addr4"), Uint128::from(40_000u32)),
        ];

        assert_eq!(allocations, expected_allocations);


        let total_amount = Uint128::from(10u8);

        let allocations = super::compute_allocations(total_amount, staker_snapshot);

        expected_allocations = vec![
            (Addr::unchecked("addr1"), Uint128::from(1u8)),
            (Addr::unchecked("addr2"), Uint128::from(2u8)),
            (Addr::unchecked("addr3"), Uint128::from(3u8)),
            (Addr::unchecked("addr4"), Uint128::from(4u8)),
        ];

        assert_eq!(allocations, expected_allocations);

        let staker_snapshot = vec![
            (Addr::unchecked("addr1"), Uint128::from(10u32)),
            (Addr::unchecked("addr2"), Uint128::from(20u32)),
            (Addr::unchecked("addr3"), Uint128::from(30u32)),
            (Addr::unchecked("addr4"), Uint128::from(40u32)),
            (Addr::unchecked("addr5"), Uint128::zero()),
        ];

        let total_amount = Uint128::from(10u8);

        let allocations = super::compute_allocations(total_amount, staker_snapshot);

        expected_allocations = vec![
            (Addr::unchecked("addr1"), Uint128::from(1u8)),
            (Addr::unchecked("addr2"), Uint128::from(2u8)),
            (Addr::unchecked("addr3"), Uint128::from(3u8)),
            (Addr::unchecked("addr4"), Uint128::from(4u8)),
            (Addr::unchecked("addr5"), Uint128::zero()),
        ];

        assert_eq!(allocations, expected_allocations);
    }
}
