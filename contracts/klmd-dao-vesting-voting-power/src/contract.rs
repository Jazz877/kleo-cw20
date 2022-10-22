#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult,
    SubMsg, Uint128, Uint256, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20Coin, TokenInfoResponse};
use cw_core_interface::voting::IsActiveResponse;
use cw_utils::parse_reply_instantiate_data;
use std::convert::TryInto;

use crate::error::ContractError;
use crate::msg::{ActiveThreshold, ActiveThresholdResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StakingInfo, TokenInfo, VestingInfo};
use crate::state::{ACTIVE_THRESHOLD, DAO, STAKING_CONTRACT, STAKING_CONTRACT_CODE_ID, STAKING_CONTRACT_UNSTAKING_DURATION, TOKEN, VESTING_CONTRACT, VESTING_CONTRACT_CODE_ID, VESTING_CONTRACT_OWNER};

pub(crate) const CONTRACT_NAME: &str = "crates.io:cw20-staked-balance-voting";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_TOKEN_REPLY_ID: u64 = 0;
const INSTANTIATE_STAKING_REPLY_ID: u64 = 1;
const INSTANTIATE_VESTING_REPLY_ID: u64 = 2;

// We multiply by this when calculating needed power for being active
// when using active threshold with percent
const PRECISION_FACTOR: u128 = 10u128.pow(9);

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    DAO.save(deps.storage, &info.sender)?;
    if let Some(active_threshold) = msg.active_threshold.clone() {
        if let ActiveThreshold::Percentage { percent } = active_threshold {
            if percent > Decimal::percent(100) || percent <= Decimal::percent(0) {
                return Err(ContractError::InvalidActivePercentage {});
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, &active_threshold)?;
    }

    match msg.token_info {
        TokenInfo::Existing {
            address,
            staking_contract,
            vesting_contract,
        } => {
            let address = deps.api.addr_validate(&address)?;
            TOKEN.save(deps.storage, &address)?;
            if let Some(ActiveThreshold::AbsoluteCount { count }) = msg.active_threshold {
                assert_valid_absolute_count_threshold(deps.as_ref(), address.clone(), count)?;
            }

            let mut staking_msg: Option<SubMsg> = None;
            let mut staking_contract_addr: Option<Addr> = None;

            match staking_contract {
                StakingInfo::Existing {
                    staking_contract_address,
                } => {
                    let staking_contract_address =
                        deps.api.addr_validate(&staking_contract_address)?;
                    let resp: cw20_stake::state::Config = deps.querier.query_wasm_smart(
                        &staking_contract_address,
                        &cw20_stake::msg::QueryMsg::GetConfig {},
                    )?;

                    if address != resp.token_address {
                        return Err(ContractError::StakingContractMismatch {});
                    }
                    STAKING_CONTRACT.save(deps.storage, &staking_contract_address)?;
                    staking_contract_addr = Some(staking_contract_address.clone());
                }
                StakingInfo::New {
                    staking_code_id,
                    unstaking_duration,
                } => {
                    let msg = WasmMsg::Instantiate {
                        code_id: staking_code_id,
                        funds: vec![],
                        admin: Some(info.sender.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&cw20_stake::msg::InstantiateMsg {
                            owner: Some(info.sender.to_string()),
                            unstaking_duration,
                            token_address: address.to_string(),
                            manager: None,
                        })?,
                    };
                    let msg = SubMsg::reply_on_success(msg, INSTANTIATE_STAKING_REPLY_ID);
                    staking_msg = Some(msg);
                }
            }
            match vesting_contract {
                VestingInfo::Existing {
                    vesting_contract_address,
                } => {
                    let vesting_contract_address =
                        deps.api.addr_validate(&vesting_contract_address)?;
                    let resp: klmd_custom_vesting::msg::TokenAddressResponse = deps.querier.query_wasm_smart(
                        &vesting_contract_address,
                        &klmd_custom_vesting::msg::QueryMsg::TokenAddress {},
                    )?;

                    if address != resp.token_address {
                        return Err(ContractError::VestingContractMismatch {});
                    }

                    VESTING_CONTRACT.save(deps.storage, &vesting_contract_address)?;

                    match staking_msg {
                        Some(msg) => {
                            Ok(Response::new().add_attribute("action", "instantiate")
                                .add_attribute("token", "existing_token")
                                .add_attribute("token_address", address)
                                .add_attribute("vesting_contract", "existing_vesting_contract")
                                .add_attribute("vesting_contract_address", vesting_contract_address)
                                .add_submessage(msg))
                        },
                        None => {
                            Ok(Response::new().add_attribute("action", "instantiate")
                                .add_attribute("token", "existing_token")
                                .add_attribute("token_address", address)
                                .add_attribute("vesting_contract", "existing_vesting_contract")
                                .add_attribute("vesting_contract_address", vesting_contract_address)
                                .add_attribute("staking_contract", "existing_staking_contract")
                                .add_attribute("staking_contract_address", staking_contract_addr.unwrap()))
                        },
                    }
                },
                VestingInfo::New {
                    vesting_code_id: vesting_contract_code_id, owner_address
                } => {
                    let owner = match owner_address {
                        Some(owner_address) => deps.api.addr_validate(&owner_address)?,
                        None => info.sender.clone()
                    };

                    let msg = WasmMsg::Instantiate {
                        code_id: vesting_contract_code_id,
                        funds: vec![],
                        admin: Some(info.sender.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&klmd_custom_vesting::msg::InstantiateMsg {
                            owner_address: Some(owner),
                            token_address: address.clone(),
                        })?,
                    };

                    let vesting_msg = SubMsg::reply_on_success(msg, INSTANTIATE_VESTING_REPLY_ID);
                    match staking_msg {
                        Some(msg) => {
                            Ok(Response::new().add_attribute("action", "instantiate")
                                .add_attribute("token", "existing_token")
                                .add_attribute("token_address", address)
                                .add_submessage(msg)
                                .add_submessage(vesting_msg))
                        },
                        None => {
                            Ok(Response::new().add_attribute("action", "instantiate")
                                .add_attribute("token", "existing_token")
                                .add_attribute("token_address", address)
                                .add_attribute("staking_contract", "existing_staking_contract")
                                .add_attribute("staking_contract_address", staking_contract_addr.unwrap())
                                .add_submessage(vesting_msg))
                        },
                    }
                }
            }
        }
        TokenInfo::New {
            code_id,
            label,
            name,
            symbol,
            decimals,
            mut initial_balances,
            initial_dao_balance,
            marketing,
            staking_code_id,
            unstaking_duration,
            vesting_code_id,
            vesting_owner_address,
        } => {
            let initial_supply = initial_balances
                .iter()
                .fold(Uint128::zero(), |p, n| p + n.amount);
            // Cannot instantiate with no initial token owners because it would immediately lock the DAO.
            if initial_supply.is_zero() {
                return Err(ContractError::InitialBalancesError {});
            }

            // Add DAO initial balance to initial_balances vector if defined.
            if let Some(initial_dao_balance) = initial_dao_balance {
                if initial_dao_balance > Uint128::zero() {
                    initial_balances.push(Cw20Coin {
                        address: info.sender.to_string(),
                        amount: initial_dao_balance,
                    });
                }
            }

            let vesting_owner = match vesting_owner_address {
                Some(vesting_owner_address) => deps.api.addr_validate(&vesting_owner_address)?,
                None => info.sender.clone()
            };

            STAKING_CONTRACT_CODE_ID.save(deps.storage, &staking_code_id)?;
            STAKING_CONTRACT_UNSTAKING_DURATION.save(deps.storage, &unstaking_duration)?;
            VESTING_CONTRACT_CODE_ID.save(deps.storage, &vesting_code_id)?;
            VESTING_CONTRACT_OWNER.save(deps.storage, &vesting_owner)?;

            let msg = WasmMsg::Instantiate {
                admin: Some(info.sender.to_string()),
                code_id,
                msg: to_binary(&cw20_base::msg::InstantiateMsg {
                    name,
                    symbol,
                    decimals,
                    initial_balances,
                    mint: Some(cw20::MinterResponse {
                        minter: info.sender.to_string(),
                        cap: None,
                    }),
                    marketing,
                })?,
                funds: vec![],
                label,
            };
            let msg = SubMsg::reply_on_success(msg, INSTANTIATE_TOKEN_REPLY_ID);

            Ok(Response::default()
                .add_attribute("action", "instantiate")
                .add_attribute("token", "new_token")
                .add_submessage(msg))
        }
    }
}

pub fn assert_valid_absolute_count_threshold(
    deps: Deps,
    token_addr: Addr,
    count: Uint128,
) -> Result<(), ContractError> {
    let token_info: cw20::TokenInfoResponse = deps
        .querier
        .query_wasm_smart(token_addr, &cw20_base::msg::QueryMsg::TokenInfo {})?;
    if count > token_info.total_supply {
        return Err(ContractError::InvalidAbsoluteCount {});
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateActiveThreshold { new_threshold } => {
            execute_update_active_threshold(deps, env, info, new_threshold)
        }
    }
}

pub fn execute_update_active_threshold(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_active_threshold: Option<ActiveThreshold>,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(active_threshold) = new_active_threshold {
        match active_threshold {
            ActiveThreshold::Percentage { percent } => {
                if percent > Decimal::percent(100) || percent <= Decimal::percent(0) {
                    return Err(ContractError::InvalidActivePercentage {});
                }
            }
            ActiveThreshold::AbsoluteCount { count } => {
                let token = TOKEN.load(deps.storage)?;
                assert_valid_absolute_count_threshold(deps.as_ref(), token, count)?;
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, &active_threshold)?;
    } else {
        ACTIVE_THRESHOLD.remove(deps.storage);
    }

    Ok(Response::new().add_attribute("action", "update_active_threshold"))
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TokenContract {} => query_token_contract(deps),
        QueryMsg::StakingContract {} => query_staking_contract(deps),
        QueryMsg::VestingContract {} => query_vesting_contract(deps),
        QueryMsg::VotingPowerAtHeight { address, height } => {
            query_voting_power_at_height(deps, env, address, height)
        }
        QueryMsg::TotalPowerAtHeight { height } => query_total_power_at_height(deps, env, height),
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::IsActive {} => query_is_active(deps),
        QueryMsg::ActiveThreshold {} => query_active_threshold(deps),
    }
}

pub fn query_token_contract(deps: Deps) -> StdResult<Binary> {
    let token = TOKEN.load(deps.storage)?;
    to_binary(&token)
}

pub fn query_staking_contract(deps: Deps) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    to_binary(&staking_contract)
}

pub fn query_vesting_contract(deps: Deps) -> StdResult<Binary> {
    let vesting_contract = VESTING_CONTRACT.load(deps.storage)?;
    to_binary(&vesting_contract)
}

pub fn query_voting_power_at_height(
    deps: Deps,
    _env: Env,
    address: String,
    height: Option<u64>,
) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    let vesting_contract = VESTING_CONTRACT.load(deps.storage)?;
    let valid_address = deps.api.addr_validate(&address)?;
    let staking_res: cw20_stake::msg::StakedBalanceAtHeightResponse = deps.querier.query_wasm_smart(
        staking_contract,
        &cw20_stake::msg::QueryMsg::StakedBalanceAtHeight {
            address: valid_address.clone().to_string(),
            height,
        },
    )?;
    let vesting_res: klmd_custom_vesting::msg::VestingAccountResponse = deps.querier.query_wasm_smart(
        vesting_contract,
        &klmd_custom_vesting::msg::QueryMsg::VestingAccount { address: valid_address.clone(), height },
    )?;
    let staking_balance = staking_res.balance;
    let vesting_balance = vesting_res.vestings.claimable_amount;
    let power = staking_balance.checked_add(vesting_balance).unwrap_or(Uint128::zero());
    to_binary(&cw_core_interface::voting::VotingPowerAtHeightResponse {
        power,
        height: staking_res.height,
    })
}

pub fn query_total_power_at_height(
    deps: Deps,
    _env: Env,
    height: Option<u64>,
) -> StdResult<Binary> {
    let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
    let vesting_contract = VESTING_CONTRACT.load(deps.storage)?;
    let staking_res: cw20_stake::msg::TotalStakedAtHeightResponse = deps.querier.query_wasm_smart(
        staking_contract,
        &cw20_stake::msg::QueryMsg::TotalStakedAtHeight { height },
    )?;
    let vesting_res: klmd_custom_vesting::msg::VestingTotalResponse = deps.querier.query_wasm_smart(
        vesting_contract,
        &klmd_custom_vesting::msg::QueryMsg::VestingTotal { height },
    )?;
    let vesting_power = vesting_res.info.vested_amount.checked_sub(vesting_res.info.claimed_amount).unwrap_or(Uint128::zero());

    let power = staking_res.total.checked_add(vesting_power).unwrap_or(Uint128::zero());

    to_binary(&cw_core_interface::voting::TotalPowerAtHeightResponse {
        power,
        height: staking_res.height,
    })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&cw_core_interface::voting::InfoResponse { info })
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let dao = DAO.load(deps.storage)?;
    to_binary(&dao)
}

pub fn query_is_active(deps: Deps) -> StdResult<Binary> {
    let threshold = ACTIVE_THRESHOLD.may_load(deps.storage)?;
    if let Some(threshold) = threshold {
        let token_contract = TOKEN.load(deps.storage)?;
        let staking_contract = STAKING_CONTRACT.load(deps.storage)?;
        let vesting_contract = VESTING_CONTRACT.load(deps.storage)?;
        let staking_power: cw20_stake::msg::TotalStakedAtHeightResponse =
            deps.querier.query_wasm_smart(
                staking_contract,
                &cw20_stake::msg::QueryMsg::TotalStakedAtHeight { height: None },
            )?;
        let vesting_response : klmd_custom_vesting::msg::VestingTotalResponse =
            deps.querier.query_wasm_smart(
                vesting_contract,
                &klmd_custom_vesting::msg::QueryMsg::VestingTotal { height: None },
            )?;
        let vesting_power = vesting_response.info.vested_amount.checked_sub(vesting_response.info.claimed_amount).unwrap_or(Uint128::zero());
        let actual_power = staking_power.total.checked_add(vesting_power).unwrap_or(Uint128::zero());
        match threshold {
            ActiveThreshold::AbsoluteCount { count } => to_binary(&IsActiveResponse {
                active: actual_power >= count,
            }),
            ActiveThreshold::Percentage { percent } => {
                let total_potential_power: TokenInfoResponse = deps
                    .querier
                    .query_wasm_smart(token_contract, &cw20_base::msg::QueryMsg::TokenInfo {})?;
                let total_power = total_potential_power
                    .total_supply
                    .full_mul(PRECISION_FACTOR);
                let applied = total_power.multiply_ratio(
                    percent.atomics(),
                    Uint256::from(10u64).pow(percent.decimal_places()),
                );
                let rounded = (applied + Uint256::from(PRECISION_FACTOR) - Uint256::from(1u128))
                    / Uint256::from(PRECISION_FACTOR);
                let count: Uint128 = rounded.try_into().unwrap();
                to_binary(&IsActiveResponse {
                    active: actual_power >= count,
                })
            }
        }
    } else {
        to_binary(&IsActiveResponse { active: true })
    }
}

pub fn query_active_threshold(deps: Deps) -> StdResult<Binary> {
    to_binary(&ActiveThresholdResponse {
        active_threshold: ACTIVE_THRESHOLD.may_load(deps.storage)?,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Set contract to version to latest
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_TOKEN_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    let token = TOKEN.may_load(deps.storage)?;
                    if token.is_some() {
                        return Err(ContractError::DuplicateToken {});
                    }
                    let token = deps.api.addr_validate(&res.contract_address)?;
                    TOKEN.save(deps.storage, &token)?;

                    let active_threshold = ACTIVE_THRESHOLD.may_load(deps.storage)?;
                    if let Some(ActiveThreshold::AbsoluteCount { count }) = active_threshold {
                        assert_valid_absolute_count_threshold(deps.as_ref(), token.clone(), count)?;
                    }

                    let staking_contract_code_id = STAKING_CONTRACT_CODE_ID.load(deps.storage)?;
                    let unstaking_duration =
                        STAKING_CONTRACT_UNSTAKING_DURATION.load(deps.storage)?;
                    let dao = DAO.load(deps.storage)?;
                    let staking_init_msg = WasmMsg::Instantiate {
                        code_id: staking_contract_code_id,
                        funds: vec![],
                        admin: Some(dao.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&cw20_stake::msg::InstantiateMsg {
                            owner: Some(dao.clone().to_string()),
                            unstaking_duration,
                            token_address: token.clone().to_string(),
                            manager: None,
                        })?,
                    };
                    let staking_init_resp = SubMsg::reply_on_success(staking_init_msg, INSTANTIATE_STAKING_REPLY_ID);

                    let vesting_contract_code_id = VESTING_CONTRACT_CODE_ID.load(deps.storage)?;
                    let vesting_owner = VESTING_CONTRACT_OWNER.may_load(deps.storage)?;

                    let owner = match vesting_owner {
                        Some(owner) => owner,
                        None => dao.clone(),
                    };

                    let vesting_init_msg = WasmMsg::Instantiate {
                        code_id: vesting_contract_code_id,
                        funds: vec![],
                        admin: Some(dao.to_string()),
                        label: env.contract.address.to_string(),
                        msg: to_binary(&klmd_custom_vesting::msg::InstantiateMsg {
                            owner_address: Some(owner),
                            token_address: token.clone(),
                        })?,
                    };
                    let vesting_init_resp = SubMsg::reply_on_success(vesting_init_msg, INSTANTIATE_VESTING_REPLY_ID);

                    Ok(Response::default()
                        .add_attribute("token_address", token)
                        .add_submessage(staking_init_resp)
                        .add_submessage(vesting_init_resp))
                }
                Err(_) => Err(ContractError::TokenInstantiateError {}),
            }
        }
        INSTANTIATE_STAKING_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    // Validate contract address
                    let staking_contract_addr = deps.api.addr_validate(&res.contract_address)?;

                    // Check if we have a duplicate
                    let staking = STAKING_CONTRACT.may_load(deps.storage)?;
                    if staking.is_some() {
                        return Err(ContractError::DuplicateStakingContract {});
                    }

                    // Save staking contract addr
                    STAKING_CONTRACT.save(deps.storage, &staking_contract_addr)?;

                    Ok(Response::new().add_attribute("staking_contract", staking_contract_addr))
                }
                Err(_) => Err(ContractError::TokenInstantiateError {}),
            }
        },
        INSTANTIATE_VESTING_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    // Validate contract address
                    let vesting_contract_addr = deps.api.addr_validate(&res.contract_address)?;

                    // Check if we have a duplicate
                    let vesting = VESTING_CONTRACT.may_load(deps.storage)?;
                    if vesting.is_some() {
                        return Err(ContractError::DuplicateVestingContract {});
                    }

                    // Save vesting contract addr
                    VESTING_CONTRACT.save(deps.storage, &vesting_contract_addr)?;

                    Ok(Response::new().add_attribute("vesting_contract", vesting_contract_addr))
                }
                Err(_) => Err(ContractError::VestingInstantiateError {}),
            }
        }
        _ => Err(ContractError::UnknownReplyId { id: msg.id }),
    }
}
