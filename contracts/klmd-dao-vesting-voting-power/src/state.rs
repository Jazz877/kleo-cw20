use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

pub const TOKEN: Item<Addr> = Item::new("token"); // cw20 token address
pub const DAO: Item<Addr> = Item::new("dao");
pub const VESTING_CONTRACT: Item<Addr> = Item::new("vesting_contract");
pub const VESTING_CONTRACT_CODE_ID: Item<u64> = Item::new("vesting_contract_code_id");
pub const VOTING_POWER_RATIO: Item<Decimal> = Item::new("voting_power_ratio");
