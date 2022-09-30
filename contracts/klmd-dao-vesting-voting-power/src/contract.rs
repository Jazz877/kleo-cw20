#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult, SubMsg, to_binary, WasmMsg};
use cw_utils::parse_reply_instantiate_data;
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, VestingInfo};
use crate::state::{DAO, TOKEN, VESTING_CONTRACT, VOTING_POWER_RATIO};

pub(crate) const CONTRACT_NAME: &str = "crates.io:klmd-dao-vesting-voting-power";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_VESTING_REPLY_ID: u64 = 0;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    DAO.save(deps.storage, &info.sender)?;
    VOTING_POWER_RATIO.save(deps.storage, &msg.voting_power_ratio)?;

    match msg.vesting_info {
        VestingInfo::Existing {
            vesting_contract_address
        } => {
            let address = deps.api.addr_validate(&vesting_contract_address)?;

            let resp: klmd_custom_vesting::msg::TokenAddressResponse = deps.querier.query_wasm_smart(
                &vesting_contract_address,
                &klmd_custom_vesting::msg::QueryMsg::TokenAddress {},
            )?;

            VESTING_CONTRACT.save(deps.storage, &address)?;
            TOKEN.save(deps.storage, &address)?;

            Ok(Response::default()
                .add_attribute("action", "instantiate")
                .add_attribute("vesting", "existing_vesting")
                .add_attribute("token_address", address)
                .add_attribute("vesting_contract", vesting_contract_address))
        },
        VestingInfo::New {
            vesting_code_id,
            token_address,
        } => {
            let token_address = deps.api.addr_validate(&token_address)?;
            let inst_msg = WasmMsg::Instantiate {
                code_id: vesting_code_id,
                funds: vec![],
                admin: Some(info.sender.to_string()),
                label: env.contract.address.to_string(),
                msg: to_binary(
                    &klmd_custom_vesting::msg::InstantiateMsg {
                        owner_address: Some(info.sender),
                        token_address: token_address,
                    }
                )?
            };
            let msg = SubMsg::reply_on_success(inst_msg, INSTANTIATE_VESTING_REPLY_ID);
            Ok(Response::default()
                .add_attribute("action", "instantiate")
                .add_attribute("vesting", "new_vesting")
                .add_attribute("token_address", &token_address)
                .add_submessage(msg))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateVotingPowerRatio { new_value } => {
            execute_update_voting_power_ratio(deps, env, info, new_value)
        }
    }
}

pub fn execute_update_voting_power_ratio(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_value: Decimal,
) -> Result<Response, ContractError> {
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    VOTING_POWER_RATIO.save(deps.storage, &new_value);

    Ok(Response::new().add_attribute("action", "update_voting_power_ratio"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TokenContract {} => query_token_contract(deps),
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

pub fn query_vesting_contract(deps: Deps) -> StdResult<Binary> {
    let vesting_addr = VESTING_CONTRACT.load(deps.storage)?;
    to_binary(&vesting_addr)
}

pub fn query_voting_power_at_height(
    deps: Deps,
    _env: Env,
    address: String,
    height: Option<u64>,
) -> StdResult<Binary> {
    let vesting_contract = VESTING_CONTRACT.load(deps.storage)?;
    let address = deps.api.addr_validate(&address)?;

    let res: cw20_stake::msg::StakedBalanceAtHeightResponse = deps.querier.query_wasm_smart(
        staking_contract,
        &cw20_stake::msg::QueryMsg::StakedBalanceAtHeight {
            address: address.to_string(),
            height,
        },
    )?;
    to_binary(&cw_core_interface::voting::VotingPowerAtHeightResponse {
        power: res.balance,
        height: res.height,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_VESTING_REPLY_ID => {
            let res = parse_reply_instantiate_data(msg);
            match res {
                Ok(res) => {
                    let vesting_contract_addr = deps.api.addr_validate(&res.contract_address)?;

                    let vesting = VESTING_CONTRACT.may_load(deps.storage)?;
                    if vesting.is_some() {
                        return Err(ContractError::DuplicateVestingContract {});
                    }

                    VESTING_CONTRACT.save(deps.storage, &vesting_contract_addr);

                    Ok(Response::new().add_attribute("vesting_contract", vesting_contract_addr)?)
                }
                Err(_) => Err(ContractError::VestingInstantiateError {}),
            }
        },
        _ => Err(ContractError::UnknownReplyId { id: msg.id }),
    }
}

