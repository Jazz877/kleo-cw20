#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::{Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, to_binary, Uint128};
use cw_utils::{Expiration, Scheduled};
use crate::error::ContractError;
use crate::state::{Config, CONFIG, LATEST_STAGE};
use cw2::set_contract_version;

pub(crate) const CONTRACT_NAME: &str = "crates.io:klmd-rev-share";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = msg
        .owner
        .map_or(Ok(info.sender), |o| deps.api.addr_validate(&o))?;

    let stage = 0;
    LATEST_STAGE.save(deps.storage, &stage)?;

    let config = Config {
        owner: Some(owner),
        cw20_token_address: deps.api.addr_validate(&msg.cw20_token_address)?,
        native_token: msg.native_token,
        cw20_staking_address: deps.api.addr_validate(&msg.cw20_staking_address)?,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            new_owner,
            new_cw20_address,
            new_native_token,
            new_cw20_staking_address,
        } => execute_update_config(
            deps,
            env,
            info,
            new_owner,
            new_cw20_address,
            new_native_token,
            new_cw20_staking_address,
        ),
        ExecuteMsg::CreateNewStage {
            total_amount,
            snapshot_block,
            expiration,
            start,
        } => execute_create_new_stage(
            deps,
            env,
            info,
            total_amount,
            snapshot_block,
            expiration,
            start,
        ),
    }
}


pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
    cw20_token_address: Option<String>,
    native_token: Option<String>,
    cw20_staking_address: Option<String>,
) -> Result<Response, ContractError> {
    // authorize owner
    let cfg = CONFIG.load(deps.storage)?;
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let current_config = CONFIG.load(deps.storage)?;

    let new_config = Config {
        owner: new_owner.map(|o| deps.api.addr_validate(&o))
            .transpose().unwrap_or(current_config.owner),
        cw20_token_address: cw20_token_address.map(|a| deps.api.addr_validate(&a))
            .transpose()?.unwrap_or(current_config.cw20_token_address),
        native_token: native_token.unwrap_or(current_config.native_token),
        cw20_staking_address: cw20_staking_address.map(|a| deps.api.addr_validate(&a))
            .transpose()?.unwrap_or(current_config.cw20_staking_address)
    };

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_create_new_stage(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    total_amount: Uint128,
    snapshot_block: Option<u64>,
    expiration: Option<Expiration>,
    start: Option<Scheduled>,
) -> Result<Response, ContractError> {
    // authorize owner
    let cfg = CONFIG.load(deps.storage)?;
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let stage = LATEST_STAGE.load(deps.storage)?;
    let new_stage = stage + 1;
    LATEST_STAGE.save(deps.storage, &new_stage)?;

    Ok(Response::new().add_attribute("action", "create_new_stage"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.map(|o| o.to_string()),
        cw20_token_address: cfg.cw20_token_address.to_string(),
        native_token: cfg.native_token,
        cw20_staking_address: cfg.cw20_staking_address.to_string(),
    })
}


#[cfg(test)]
mod tests {
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw_multi_test::App;
    use crate::contract::{execute, instantiate, query};
    use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

    fn mock_app() -> App {
        App::default()
    }

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "anchor0000".to_string(),
            native_token: "ujunox".to_string(),
            cw20_staking_address: "staking0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0000", config.owner.unwrap().as_str());
        assert_eq!("anchor0000", config.cw20_token_address);
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("staking0000", config.cw20_staking_address.as_str());

        // let res = query(deps.as_ref(), env, QueryMsg::LatestStage {}).unwrap();
        // let latest_stage: LatestStageResponse = from_binary(&res).unwrap();
        // assert_eq!(0u8, latest_stage.latest_stage);
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "anchor0000".to_string(),
            native_token: "junox".to_string(),
            cw20_staking_address: "staking0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // update owner
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: Some("owner0001".to_string()),
            new_cw20_address: Some("cw20_0000".to_string()),
            new_native_token: Some("native_0000".to_string()),
            new_cw20_staking_address: Some("cw20_staking_0000".to_string()),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // // it worked, let's query the state
        // let res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        // let config: ConfigResponse = from_binary(&res).unwrap();
        // assert_eq!("owner0001", config.owner.unwrap().as_str());
        // assert_eq!("cw20_0000", config.cw20_token_address.unwrap().as_str());
        //
        // // Unauthorized err
        // let env = mock_env();
        // let info = mock_info("owner0000", &[]);
        // let msg = ExecuteMsg::UpdateConfig {
        //     new_owner: None,
        //     new_cw20_address: Some("cw20_0001".to_string()),
        //     new_native_token: None,
        // };
        //
        // let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        // assert_eq!(res, ContractError::Unauthorized {});
        //
        // //update with native token
        // let env = mock_env();
        // let info = mock_info("owner0001", &[]);
        // let msg = ExecuteMsg::UpdateConfig {
        //     new_owner: Some("owner0001".to_string()),
        //     new_cw20_address: None,
        //     new_native_token: Some("ujunox".to_string()),
        // };
        //
        // let _res = execute(deps.as_mut(), env.clone(), info, msg).ok();
        //
        // let query_result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        // let config: ConfigResponse = from_binary(&query_result).unwrap();
        // assert_eq!("owner0001", config.owner.unwrap().as_str());
        // assert_eq!("ujunox", config.native_token.unwrap().as_str());
        //
        // //update cw20_address and native token together
        // let env = mock_env();
        // let info = mock_info("owner0001", &[]);
        // let msg = ExecuteMsg::UpdateConfig {
        //     new_owner: Some("owner0001".to_string()),
        //     new_cw20_address: Some("cw20_0001".to_string()),
        //     new_native_token: Some("ujunox".to_string()),
        // };
        //
        // let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        // assert_eq!(res, ContractError::InvalidTokenType {});
    }
}


