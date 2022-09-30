use cosmwasm_std::Decimal;
use cw_core_macros::{active_query, token_query, voting_query};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VestingInfo {
    Existing {
        vesting_contract_address: String,
    },
    New {
        vesting_code_id: u64,
        token_address: String,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vesting_info: VestingInfo,
    pub voting_power_ratio: Decimal, // (Decimal(1_000_000_000_000_000_000) == 1.0) conversion ratio for vesting token (e.g.: 1 token = 1 voting power)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateVotingPowerRatio {
        new_value: Decimal, // Decimal(1_000_000_000_000_000_000) == 1.0
    }
}

#[active_query]
#[voting_query]
#[token_query]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    VestingContract {},
    Dao {},
    VotingPowerRatio {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct VotingPowerRatioResponse {
    pub voting_power_ratio: Decimal,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MigrateMsg {}



