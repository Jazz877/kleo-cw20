use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw_utils::{Expiration, Scheduled};

#[cw_serde]
pub struct InstantiateMsg {
    /// Owner if none set to info.sender.
    pub owner: Option<String>,
    pub cw20_token_address: String,
    pub native_token: String,
    pub cw20_staking_address: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        /// NewOwner if non sent, contract gets locked. Recipients can receive airdrops
        /// but owner cannot register new stages.
        new_owner: Option<String>,
        new_cw20_address: Option<String>,
        new_native_token: Option<String>,
        new_cw20_staking_address: Option<String>,
    },
    LockContract {},
    CreateNewStage {
        total_amount: Uint128,
        snapshot_block: Option<u64>,
        expiration: Option<Expiration>,
        start: Option<Scheduled>,
        stakers_pagination_limit: Option<u32>,
    },
    Claim {
        stage: u8,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(LatestStageResponse)]
    LatestStage {},
    #[returns(AllAllocationsResponse)]
    AllAllocations {
        stage: u8,
    },
    #[returns(IsClaimedResponse)]
    IsClaimed {
        stage: u8,
        address: String,
    },
    #[returns(TotalClaimedResponse)]
    TotalClaimed {
        stage: u8
    },
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: Option<String>,
    pub cw20_token_address: String,
    pub native_token: String,
    pub cw20_staking_address: String,
}

#[cw_serde]
pub struct LatestStageResponse {
    pub latest_stage: u8,
}

#[cw_serde]
pub struct AllocationResponse {
    pub address: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct AllAllocationsResponse {
    pub allocations: Vec<AllocationResponse>,
}

#[cw_serde]
pub struct IsClaimedResponse {
    pub is_claimed: bool,
}

#[cw_serde]
pub struct TotalClaimedResponse {
    pub total_claimed: Uint128,
}
