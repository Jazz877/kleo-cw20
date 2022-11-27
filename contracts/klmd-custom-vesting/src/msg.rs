use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw2::ContractVersion;
use proposal_hooks::ProposalHookMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{TotalVestingInfo, VestingData};

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
        prevesting_amount: Uint128,
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
    ProposalHook(ProposalHookMsg),
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
    Info {},
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

#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
pub struct InfoResponse {
    pub info: ContractVersion,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
}
