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
        /// if not specified, no update
        new_owner: Option<String>,
        /// if not specified, no update
        new_cw20_address: Option<String>,
        /// if not specified, no update
        new_native_token: Option<String>,
        /// if not specified, no update
        new_cw20_staking_address: Option<String>,
    },
    LockContract {},
    CreateNewStage {
        total_amount: Uint128,
        snapshot_block: Option<u64>,
        expiration: Option<Expiration>,
        start: Option<Scheduled>,
    },
    Claim {
        stage: u8,
    },
    Pause {
        stage: u8,
    },
    /// Withdraw the remaining tokens in the stage after expiry time (only owner)
    Withdraw {
        stage: u8,
        address: String,
    },
    /// Withdraw all/some of the remaining tokens that the contract owns (only owner)
    WithdrawAll {
        address: String,
        amount: Option<Uint128>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(LatestStageResponse)]
    LatestStage {},
    #[returns(StageBlockResponse)]
    StageBlock { stage: u8 },
    #[returns(IsClaimedResponse)]
    IsClaimed {
        stage: u8,
        address: String,
    },
    #[returns(TotalClaimedResponse)]
    TotalClaimed {
        stage: u8
    },
    #[returns(IsPausedResponse)]
    IsPaused { stage: u8 },
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
pub struct StageBlockResponse {
    pub stage_block: u64,
}

#[cw_serde]
pub struct IsClaimedResponse {
    pub is_claimed: bool,
}

#[cw_serde]
pub struct TotalClaimedResponse {
    pub total_claimed: Uint128,
}

#[cw_serde]
pub struct IsPausedResponse {
    pub is_paused: bool,
}
