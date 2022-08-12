use crate::state::PaymentState;
use cosmwasm_std::{Addr, Uint128};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub schedule: Vec<Payment>,
    pub owner_address: Option<Addr>,
    pub token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Payment {
    pub recipient: Addr,
    pub amount: Uint128,
    pub denom: String,
    pub time: Expiration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Pay {},
    RevokePayments {
        payment_ids: Vec<u64>
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetPayments {},
    GetOwner {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PaymentsResponse {
    pub payments: Vec<PaymentState>,
}
