use cosmwasm_std::{Addr, Uint128, Timestamp};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InstantiateMsg {
    pub owner_address: Option<Addr>,
    pub token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateOwnerAddress {
        address: Addr,
    },
    RegisterVestingAccount {
        address: Addr,
        vesting_amount: Uint128,
        start_time: Timestamp,
        end_time: Timestamp,
    },
    DeregisterVestingAccount {
        address: Addr,
        vested_token_recipient: Option<Addr>,
        left_vesting_token_recipient: Option<Addr>,
    },
    Claim {
        recipient: Option<Addr>,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    OwnerAddress {},
    TokenAddress {},
    VestingAccount {
        address: Addr,
        start_after: Option<Addr>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct OwnerAddressResponse {
    pub owner_address: Addr,
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct TokenAddressResponse {
    pub token_address: Addr,
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct VestingAccountResponse {
    pub address: Addr,
    pub vestings: Vec<VestingData>,
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct VestingData {
    pub vesting_amount: Uint128,
    pub vested_amount: Uint128,
    pub claimable_amount: Uint128,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
}
