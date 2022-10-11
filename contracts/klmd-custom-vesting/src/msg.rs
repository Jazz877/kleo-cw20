use cosmwasm_std::{Addr, Uint128, Timestamp};
use proposal_hooks::ProposalHookMsg;
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

use crate::state::{VestingData, TotalVestingInfo};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
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
    },
    Snapshot {},
    ProposalHookMsg(ProposalHookMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    OwnerAddress {},
    TokenAddress {},
    VestingAccount {
        address: Addr,
        height: Option<u64>,
    },
    VestingTotal {
        height: Option<u64>,
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
    pub vestings: VestingData,
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct VestingTotalResponse {
    pub info: TotalVestingInfo,
}