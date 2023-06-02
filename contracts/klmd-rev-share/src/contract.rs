#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{Addr, attr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Storage, to_binary, Uint128, Uint64};
use cw20_stake::msg::{StakedBalanceAtHeightResponse, TotalStakedAtHeightResponse};
use cw2::set_contract_version;
use cw_utils::{Expiration, Scheduled};

use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, IsClaimedResponse, IsPausedResponse, LatestStageResponse, QueryMsg, StageBlockResponse, TotalClaimedResponse};
use crate::state::{CLAIM, compute_allocation, Config, CONFIG, LATEST_STAGE, STAGE_AMOUNT, STAGE_AMOUNT_CLAIMED, STAGE_EXPIRATION, STAGE_HEIGHT, STAGE_PAUSED, STAGE_START};

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
        ExecuteMsg::Pause { stage } => execute_pause(deps, env, info, stage),
        ExecuteMsg::Claim { stage } => execute_claim(deps, env, info, stage),
        ExecuteMsg::LockContract {} => execute_lock_contract(deps, env, info),
        ExecuteMsg::Withdraw {
            stage,
            address,
        } => execute_withdraw(deps, env, info, stage, address),
        ExecuteMsg::WithdrawAll {
            address,
            amount
        } => execute_withdraw_all(deps, env, info, address, amount)
    }
}

fn only_owner(storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
    let cfg = CONFIG.load(storage)?;
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if owner != sender {
        return Err(ContractError::Unauthorized {});
    }

    Ok(())
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
    only_owner(deps.storage, &info.sender)?;

    let current_config = CONFIG.load(deps.storage)?;

    let _owner = new_owner.map(|o| deps.api.addr_validate(&o))
        .transpose()?;

    let new_config = Config {
        owner: _owner.is_some().then(|| _owner.unwrap()).or_else(|| current_config.owner.clone()),
        cw20_token_address: cw20_token_address.map(|a| deps.api.addr_validate(&a))
            .transpose()?.unwrap_or(current_config.cw20_token_address),
        native_token: native_token.unwrap_or(current_config.native_token),
        cw20_staking_address: cw20_staking_address.map(|a| deps.api.addr_validate(&a))
            .transpose()?.unwrap_or(current_config.cw20_staking_address),
    };

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_create_new_stage(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    total_amount: Uint128,
    snapshot_block: Option<u64>,
    expiration: Option<Expiration>,
    start: Option<Scheduled>,
) -> Result<Response, ContractError> {
    // authorize owner
    only_owner(deps.storage, &info.sender)?;

    let stage = LATEST_STAGE.update(deps.storage, |stage| -> StdResult<_> { Ok(stage + 1) })?;

    let stage_block = match snapshot_block {
        Some(block) => block,
        None => env.block.height,
    };
    // save snapshot block
    STAGE_HEIGHT.save(deps.storage, stage, &Uint64::from(stage_block))?;

    // save expiration
    let exp = expiration.unwrap_or(Expiration::Never {});
    STAGE_EXPIRATION.save(deps.storage, stage, &exp)?;

    // save start
    if let Some(start) = start {
        STAGE_START.save(deps.storage, stage, &start)?;
    }

    STAGE_PAUSED.save(deps.storage, stage, &false)?;

    // save total airdropped amount
    STAGE_AMOUNT.save(deps.storage, stage, &total_amount)?;
    STAGE_AMOUNT_CLAIMED.save(deps.storage, stage, &Uint128::zero())?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "create_new_stage"),
        attr("stage", stage.to_string()),
        attr("total_amount", total_amount),
    ]))
}

pub fn execute_pause(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stage: u8,
) -> Result<Response, ContractError> {
    // authorize owner
    only_owner(deps.storage, &info.sender)?;

    let start = STAGE_START.may_load(deps.storage, stage)?;
    if let Some(start) = start {
        if !start.is_triggered(&env.block) {
            return Err(ContractError::StageNotBegun { stage, start });
        }
    }

    let expiration = STAGE_EXPIRATION.load(deps.storage, stage)?;
    if expiration.is_expired(&env.block) {
        return Err(ContractError::StageExpired { stage, expiration });
    }

    STAGE_PAUSED.save(deps.storage, stage, &true)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "pause"),
        attr("stage", stage.to_string()),
        attr("stage_paused", "true"),
    ]))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stage: u8,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let start = STAGE_START.may_load(deps.storage, stage)?;
    if let Some(start) = start {
        if !start.is_triggered(&env.block) {
            return Err(ContractError::StageNotBegun { stage, start });
        }
    }

    let expiration = STAGE_EXPIRATION.load(deps.storage, stage)?;
    if expiration.is_expired(&env.block) {
        return Err(ContractError::StageExpired { stage, expiration });
    }

    let is_paused = STAGE_PAUSED.load(deps.storage, stage)?;
    if is_paused {
        return Err(ContractError::StagePaused { stage });
    }

    let claimed = CLAIM.may_load(deps.storage, (info.sender.to_string().into(), stage))?;
    if claimed.is_some() {
        return Err(ContractError::Claimed {});
    }

    let stage_amount = STAGE_AMOUNT.load(deps.storage, stage)?;
    let stage_block = STAGE_HEIGHT.load(deps.storage, stage)?;

    let total_staked_response: TotalStakedAtHeightResponse = deps.querier.query_wasm_smart(
        config.cw20_staking_address.clone(),
        &cw20_stake::msg::QueryMsg::TotalStakedAtHeight {
            height: Some(stage_block.into()),
        },
    )?;

    let address_staked_response: StdResult<StakedBalanceAtHeightResponse> = deps.querier.query_wasm_smart(
        config.cw20_staking_address,
        &cw20_stake::msg::QueryMsg::StakedBalanceAtHeight {
            address: info.sender.to_string(),
            height: Some(stage_block.into()),
        },
    );

    // if address has no stake, return error
    match address_staked_response {
        Ok(response) => {
            let amount = response.balance;
            if amount == Uint128::zero() {
                return Err(ContractError::NoStake {});
            }

            let addr_allocation = compute_allocation(
                stage_amount,
                total_staked_response.total,
                amount,
            );

            // Update total claimed to reflect
            let mut claimed_amount = STAGE_AMOUNT_CLAIMED.load(deps.storage, stage)?;
            claimed_amount += addr_allocation;
            STAGE_AMOUNT_CLAIMED.save(deps.storage, stage, &claimed_amount)?;

            CLAIM.save(deps.storage, (info.sender.to_string().into(), stage), &true)?;

            // send native tokens
            let balance = deps
                .querier
                .query_balance(env.contract.address, config.native_token.clone())?;

            if balance.amount < addr_allocation {
                return Err(ContractError::InsufficientFunds {
                    balance: balance.amount,
                    amount: addr_allocation,
                });
            }

            let msg = BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: config.native_token,
                    amount: addr_allocation,
                }],
            };
            let cosmos_msg = CosmosMsg::Bank(msg);

            let res = Response::new().add_message(cosmos_msg).add_attributes(vec![
                attr("action", "claim"),
                attr("stage", stage.to_string()),
                attr("address", info.sender.to_string()),
                attr("amount", addr_allocation),
            ]);
            Ok(res)
        }
        Err(_) => {
            return Err(ContractError::NoStake {});
        }
    }
}

pub fn execute_lock_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    only_owner(deps.storage, &info.sender)?;
    let config = CONFIG.load(deps.storage)?;

    let new_config = Config {
        owner: None,
        cw20_token_address: config.cw20_token_address,
        native_token: config.native_token,
        cw20_staking_address: config.cw20_staking_address,
    };

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::new().add_attribute("action", "lock_contract"))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stage: u8,
    address: String,
) -> Result<Response, ContractError> {
    // authorize owner
    only_owner(deps.storage, &info.sender)?;

    let cfg = CONFIG.load(deps.storage)?;

    // make sure is expired if the stage is not paused
    let is_paused = STAGE_PAUSED.load(deps.storage, stage)?;
    if !is_paused {
        let expiration = STAGE_EXPIRATION.load(deps.storage, stage)?;
        if !expiration.is_expired(&env.block) {
            return Err(ContractError::StageNotExpired { stage, expiration });
        }
    }

    // Get total amount per stage and total claimed
    let total_amount = STAGE_AMOUNT.load(deps.storage, stage)?;
    let claimed_amount = STAGE_AMOUNT_CLAIMED.load(deps.storage, stage)?;

    // impossible but who knows
    if claimed_amount > total_amount {
        return Err(ContractError::Unauthorized {});
    }

    // Get balance
    let balance_to_withdraw = total_amount - claimed_amount;

    // Validate address
    let recipient = deps.api.addr_validate(&address)?;

    // Withdraw the tokens and response
    let balance = deps
        .querier
        .query_balance(env.contract.address, &cfg.native_token)?;
    if balance.amount < balance_to_withdraw {
        return Err(ContractError::InsufficientFunds {
            balance: balance.amount,
            amount: balance_to_withdraw,
        });
    }
    let msg = BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: cfg.native_token,
            amount: balance_to_withdraw,
        }],
    };
    let message = CosmosMsg::Bank(msg);
    let res = Response::new().add_message(message).add_attributes(vec![
        attr("action", "withdraw"),
        attr("stage", stage.to_string()),
        attr("address", info.sender),
        attr("amount", balance_to_withdraw),
        attr("recipient", address),
    ]);

    Ok(res)
}

pub fn execute_withdraw_all(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    // authorize owner
    only_owner(deps.storage, &info.sender)?;

    let cfg = CONFIG.load(deps.storage)?;

    // make sure all the stages are either paused or expired
    let latest_stage = LATEST_STAGE.load(deps.storage)?;
    for stage_id in 1..=latest_stage {
        let is_paused = STAGE_PAUSED.load(deps.storage, stage_id)?;
        if !is_paused {
            let expiration = STAGE_EXPIRATION.load(deps.storage, stage_id)?;
            if !expiration.is_expired(&env.block) {
                return Err(ContractError::StageNotExpired {
                    stage: stage_id,
                    expiration,
                });
            }
        }
    }

    // Get the current total balance for the contract
    let total_amount = deps.querier
        .query_balance(env.contract.address, &cfg.native_token)?
        .amount;

    let amount_to_withdraw = match amount {
        Some(amount) => {
            if amount > total_amount {
                return Err(ContractError::InsufficientFunds {
                    balance: total_amount,
                    amount,
                });
            }
            amount
        }
        None => total_amount,
    };

    // Validate address
    let recipient = deps.api.addr_validate(&address)?;

    let msg = BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: cfg.native_token,
            amount: amount_to_withdraw,
        }],
    };
    let message = CosmosMsg::Bank(msg);

    let res = Response::new().add_message(message).add_attributes(vec![
        attr("action", "withdraw_all"),
        attr("address", info.sender),
        attr("amount", amount_to_withdraw),
        attr("recipient", address),
    ]);
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::LatestStage {} => to_binary(&query_latest_stage(deps)?),
        QueryMsg::StageBlock { stage } => to_binary(&query_stage_block(deps, stage)?),
        QueryMsg::IsClaimed { stage, address } => to_binary(&query_is_claimed(deps, stage, address)?),
        QueryMsg::TotalClaimed { stage } => to_binary(&query_total_claimed(deps, stage)?),
        QueryMsg::IsPaused { stage } => to_binary(&query_is_paused(deps, stage)?),
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

pub fn query_latest_stage(deps: Deps) -> StdResult<LatestStageResponse> {
    let latest_stage = LATEST_STAGE.load(deps.storage)?;
    let resp = LatestStageResponse { latest_stage };

    Ok(resp)
}

pub fn query_stage_block(deps: Deps, stage: u8) -> StdResult<StageBlockResponse> {
    let stage_block = STAGE_HEIGHT.load(deps.storage, stage)?.u64();

    Ok(StageBlockResponse { stage_block })
}

pub fn query_is_claimed(deps: Deps, stage: u8, address: String) -> StdResult<IsClaimedResponse> {
    let is_claimed = CLAIM
        .may_load(deps.storage, (address.into(), stage))?
        .unwrap_or(false);
    Ok(IsClaimedResponse { is_claimed })
}

pub fn query_total_claimed(deps: Deps, stage: u8) -> StdResult<TotalClaimedResponse> {
    let total_claimed = STAGE_AMOUNT_CLAIMED.load(deps.storage, stage)?;
    Ok(TotalClaimedResponse { total_claimed })
}

pub fn query_is_paused(deps: Deps, stage: u8) -> StdResult<IsPausedResponse> {
    let is_paused = STAGE_PAUSED.may_load(deps.storage, stage)?.unwrap_or(false);
    Ok(IsPausedResponse { is_paused })
}


#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, BalanceResponse, BankMsg, BankQuery, BlockInfo, Coin, CosmosMsg, Empty, from_binary, Querier, QueryRequest, Response, SubMsg, to_binary, Uint128};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw20::Cw20Coin;
    use cw_multi_test::{App, Contract, ContractWrapper, Executor, next_block};
    use cw_utils::{Expiration, Scheduled};

    use crate::contract::{execute, instantiate, query};
    use crate::error::ContractError;
    use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, IsClaimedResponse, IsPausedResponse, LatestStageResponse, QueryMsg, StageBlockResponse, TotalClaimedResponse};

    fn mock_app() -> App {
        App::default()
    }

    fn jump_100_blocks(block: &mut BlockInfo) {
        block.time = block.time.plus_seconds(500);
        block.height += 100;
    }

    fn mint_native(app: &mut App, beneficiary: String, denom: String, amount: Uint128) {
        app.sudo(cw_multi_test::SudoMsg::Bank(
            cw_multi_test::BankSudo::Mint {
                to_address: beneficiary,
                amount: vec![Coin { amount, denom }],
            },
        ))
            .unwrap();
    }

    fn query_balance_native(app: &App, address: &Addr, denom: String) -> Coin {
        let req: QueryRequest<BankQuery> = QueryRequest::Bank(BankQuery::Balance { address: address.to_string(), denom: denom.to_string() });
        let res = app.raw_query(&to_binary(&req).unwrap()).unwrap().unwrap();
        let balance: BalanceResponse = from_binary(&res).unwrap();

        return balance.amount;
    }

    fn cw20_token_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        );
        Box::new(contract)
    }

    fn staking_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_stake::contract::execute,
            cw20_stake::contract::instantiate,
            cw20_stake::contract::query,
        );
        Box::new(contract)
    }

    fn rev_share_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            execute,
            instantiate,
            query,
        );
        Box::new(contract)
    }

    fn admin() -> Addr {
        Addr::unchecked("admin")
    }

    fn user(name: &str) -> Addr {
        Addr::unchecked(name)
    }

    fn instantiate_staking(
        app: &mut App, cw20_balances: Vec<Cw20Coin>,
    ) -> (Addr, Addr) {
        let admin = admin();
        let cw20_token_contract_id = app.store_code(cw20_token_contract());
        let cw20_token_contract_addr = app.instantiate_contract(
            cw20_token_contract_id,
            admin.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Wonderful Token".to_string(),
                symbol: "WNDT".to_string(),
                decimals: 6,
                initial_balances: cw20_balances,
                mint: None,
                marketing: None,
            },
            &[],
            "cw20 token",
            admin.clone().to_string().into(),
        ).unwrap();


        let staking_contract_id = app.store_code(staking_contract());

        let staking_contract_addr = app.instantiate_contract(
            staking_contract_id,
            admin.clone().into(),
            &cw20_stake::msg::InstantiateMsg {
                owner: admin.clone().to_string().into(),
                token_address: cw20_token_contract_addr.to_string().into(),
                unstaking_duration: None,
            },
            &[],
            "staking contract",
            admin.to_string().into(),
        )
            .unwrap();

        return (cw20_token_contract_addr, staking_contract_addr);
    }

    fn initialize_staking_contract_status(
        app: &mut App,
        staking_contract_addr: &Addr,
        cw20_token_contract_addr: &Addr,
        stakers: Vec<(&Addr, u128)>,
    ) {
        for (addr, amount) in stakers {
            stake_tokens(
                app,
                staking_contract_addr.clone(),
                cw20_token_contract_addr.clone(),
                addr.as_str(),
                amount,
            );
        }
    }

    fn initialize_rev_share_contract(app: &mut App) -> Addr {
        let admin = admin();
        let user1 = user("user0001");
        let user2 = user("user0002");
        let (cw20_contract_addr, staking_contract_addr) = instantiate_staking(
            app,
            vec![
                Cw20Coin {
                    address: admin.clone().to_string().into(),
                    amount: Uint128::from(1000_000_000u128),
                },
                Cw20Coin {
                    address: user("user0001").to_string().into(),
                    amount: Uint128::from(1000_000_000u128),
                },
                Cw20Coin {
                    address: user("user0002").to_string().into(),
                    amount: Uint128::from(1000_000_000u128),
                },
            ]
        );

        // user1 and user2 stake 1000 cw20 tokens to staking contract
        initialize_staking_contract_status(
            app,
            &staking_contract_addr,
            &cw20_contract_addr,
            vec![
                (&user1, 1000_000_000u128),
                (&user2, 1000_000_000u128),
            ],
        );

        // create rev share contract

        let inst_msg = InstantiateMsg {
            owner: Some(admin.clone().to_string().into()),
            cw20_token_address: cw20_contract_addr.clone().to_string().into(),
            native_token: "ujunox".to_string(),
            cw20_staking_address: staking_contract_addr.clone().to_string().into(),
        };

        let rev_share_contract_id = app.store_code(rev_share_contract());

        let rev_share_contract_addr = app.instantiate_contract(
            rev_share_contract_id,
            admin.clone(),
            &inst_msg,
            &[],
            "rev share contract",
            admin.clone().to_string().into(),
        ).unwrap();

        rev_share_contract_addr
    }

    fn stake_tokens(app: &mut App, staking_addr: Addr, cw20_addr: Addr, sender: &str, amount: u128) {
        let msg = cw20::Cw20ExecuteMsg::Send {
            contract: staking_addr.to_string(),
            amount: Uint128::new(amount),
            msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
        };
        app.execute_contract(Addr::unchecked(sender), cw20_addr, &msg, &[])
            .unwrap();
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

        let res = query(deps.as_ref(), env, QueryMsg::LatestStage {}).unwrap();
        let latest_stage: LatestStageResponse = from_binary(&res).unwrap();
        assert_eq!(0u8, latest_stage.latest_stage);
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
            new_cw20_address: Some("anchor0001".to_string()),
            new_native_token: Some("utoken".to_string()),
            new_cw20_staking_address: Some("staking0001".to_string()),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0001", config.owner.unwrap().as_str());
        assert_eq!("anchor0001", config.cw20_token_address.as_str());
        assert_eq!("utoken", config.native_token.as_str());
        assert_eq!("staking0001", config.cw20_staking_address.as_str());

        // Unauthorized err
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: None,
            new_cw20_address: Some("cw20_0001".to_string()),
            new_native_token: None,
            new_cw20_staking_address: None,
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        //update only native token
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: None,
            new_cw20_address: None,
            new_native_token: Some("ujunox".to_string()),
            new_cw20_staking_address: None,
        };

        let _res = execute(deps.as_mut(), env.clone(), info, msg).ok();

        let query_result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&query_result).unwrap();
        assert_eq!("owner0001", config.owner.unwrap().as_str());
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("anchor0001", config.cw20_token_address.as_str());
        assert_eq!("staking0001", config.cw20_staking_address.as_str());

        // update only cw20_address
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: None,
            new_cw20_address: Some("cw20_0001".to_string()),
            new_native_token: None,
            new_cw20_staking_address: None,
        };

        let _res = execute(deps.as_mut(), env.clone(), info, msg).ok();

        let query_result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&query_result).unwrap();
        assert_eq!("owner0001", config.owner.unwrap().as_str());
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("cw20_0001", config.cw20_token_address.as_str());
        assert_eq!("staking0001", config.cw20_staking_address.as_str());

        // update only cw20_staking_address
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: None,
            new_cw20_address: None,
            new_native_token: None,
            new_cw20_staking_address: Some("staking0002".to_string()),
        };

        let _res = execute(deps.as_mut(), env.clone(), info, msg).ok();

        let query_result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&query_result).unwrap();
        assert_eq!("owner0001", config.owner.unwrap().as_str());
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("cw20_0001", config.cw20_token_address.as_str());
        assert_eq!("staking0002", config.cw20_staking_address.as_str());

        // update only owner
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: "owner0002".to_string().into(),
            new_cw20_address: None,
            new_native_token: None,
            new_cw20_staking_address: None,
        };

        let _res = execute(deps.as_mut(), env.clone(), info, msg).ok();

        let query_result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();

        let config: ConfigResponse = from_binary(&query_result).unwrap();

        assert_eq!("owner0002", config.owner.unwrap().as_str());
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("cw20_0001", config.cw20_token_address.as_str());
        assert_eq!("staking0002", config.cw20_staking_address.as_str());
    }

    #[test]
    fn lock_contract() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "anchor0000".to_string(),
            native_token: "ujunox".to_string(),
            cw20_staking_address: "staking0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0000", config.owner.unwrap().as_str());
        assert_eq!("anchor0000", config.cw20_token_address);
        assert_eq!("ujunox", config.native_token.as_str());
        assert_eq!("staking0000", config.cw20_staking_address.as_str());

        // lock contract
        let msg = ExecuteMsg::LockContract {};
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(None, config.owner);

        // fail update config

        let info = mock_info("owner0000", &[]);
        let env = mock_env();

        let msg = ExecuteMsg::UpdateConfig {
            new_owner: None,
            new_cw20_address: Some("anchor0001".to_string()),
            new_native_token: None,
            new_cw20_staking_address: None,
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});
    }

    #[test]
    fn create_new_stage() {
        let admin = admin();
        let user1 = user("user0001");

        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: None,
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );

        // query latest stage
        let msg = QueryMsg::LatestStage {};

        let latest_stage_response: LatestStageResponse = app
            .wrap()
            .query_wasm_smart(rev_share_contract_addr.clone(), &msg)
            .unwrap();

        assert_eq!(latest_stage_response, LatestStageResponse { latest_stage: 1 });

        // query stage block height
        let msg = QueryMsg::StageBlock {
            stage: 1,
        };

        let stage_block_height_response: StageBlockResponse = app
            .wrap()
            .query_wasm_smart(rev_share_contract_addr.clone(), &msg)
            .unwrap();

        assert_eq!(stage_block_height_response, StageBlockResponse { stage_block: app.block_info().height });

        // user1 claims half of his allocation
        let msg = ExecuteMsg::Claim {
            stage: 1,
        };

        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query is_claimed and total_claimed
        let msg = QueryMsg::IsClaimed {
            stage: 1,
            address: user1.clone().to_string(),
        };

        let is_claimed_response: IsClaimedResponse = app
            .wrap()
            .query_wasm_smart(rev_share_contract_addr.clone(), &msg)
            .unwrap();

        assert_eq!(is_claimed_response, IsClaimedResponse { is_claimed: true });

        let msg = QueryMsg::TotalClaimed {
            stage: 1,
        };

        let total_claimed_response: TotalClaimedResponse = app
            .wrap()
            .query_wasm_smart(rev_share_contract_addr.clone(), &msg)
            .unwrap();

        assert_eq!(
            total_claimed_response,
            TotalClaimedResponse { total_claimed: Uint128::from(100_000_000u128) }
        );

        // pause stage and then withdraw leftover tokens
        let msg = ExecuteMsg::Pause {
            stage: 1,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query is_paused
        let msg = QueryMsg::IsPaused {
            stage: 1,
        };

        let is_paused_response: IsPausedResponse = app
            .wrap()
            .query_wasm_smart(rev_share_contract_addr.clone(), &msg)
            .unwrap();

        assert_eq!(is_paused_response, IsPausedResponse { is_paused: true });

        // withdraw leftover tokens

        let msg = ExecuteMsg::Withdraw {
            stage: 1,
            address: admin.clone().to_string(),
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query balance of admin

        let admin_balance = query_balance_native(
            &mut app,
            &admin,
            "ujunox".to_string(),
        );

        assert_eq!(admin_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::from(100_000_000u128) });

        // query balance of rev share contract

        let rev_share_contract_balance = query_balance_native(
            &mut app,
            &rev_share_contract_addr,
            "ujunox".to_string(),
        );

        assert_eq!(rev_share_contract_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::zero() });
    }

    #[test]
    fn can_withdraw() {
        let admin = admin();
        let user1 = user("user0001");
        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(Expiration::AtHeight(app.block_info().height + 100)),
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };
        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // move 100 blocks forward
        app.update_block(jump_100_blocks);


        // Can withdraw after expired stage

        let msg = ExecuteMsg::Withdraw {
            stage: 1,
            address: admin.clone().to_string(),
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query balance of admin

        let admin_balance = query_balance_native(
            &mut app,
            &admin,
            "ujunox".to_string(),
        );

        assert_eq!(admin_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::from(100_000_000u128) });

        // query balance of rev share contract

        let rev_share_contract_balance = query_balance_native(
            &mut app,
            &rev_share_contract_addr,
            "ujunox".to_string(),
        );

        assert_eq!(rev_share_contract_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::zero() });
    }

    #[test]
    fn cant_withdraw() {
        let admin = admin();
        let user1 = user("user0001");
        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let stage_expiration = Expiration::AtHeight(app.block_info().height + 100);
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(stage_expiration),
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };
        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // move 1 block forward
        app.update_block(next_block);


        // Can withdraw after expired stage

        let msg = ExecuteMsg::Withdraw {
            stage: 1,
            address: admin.clone().to_string(),
        };

        let error = app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap_err();

        assert_eq!(
            error.downcast::<ContractError>().unwrap(),
            ContractError::StageNotExpired { stage: 1u8, expiration: stage_expiration }
        );
    }

    #[test]
    fn can_withdraw_all() {
        let admin = admin();
        let user1 = user("user0001");
        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(Expiration::AtHeight(app.block_info().height + 100)),
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );

        // create new stage2
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(100_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(Expiration::AtHeight(app.block_info().height + 95)),
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(100_000_000u128),
        );

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };
        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // move 100 blocks forward
        app.update_block(jump_100_blocks);


        // Can withdraw after expired stage

        let msg = ExecuteMsg::WithdrawAll {
            address: admin.clone().to_string(),
            amount: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query balance of admin

        let admin_balance = query_balance_native(
            &mut app,
            &admin,
            "ujunox".to_string(),
        );

        assert_eq!(admin_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::from(200_000_000u128) });

        // query balance of rev share contract

        let rev_share_contract_balance = query_balance_native(
            &mut app,
            &rev_share_contract_addr,
            "ujunox".to_string(),
        );

        assert_eq!(rev_share_contract_balance, Coin { denom: "ujunox".to_string(), amount: Uint128::zero() });
    }

    #[test]
    fn stage_starts() {
        let admin = admin();
        let user1 = user("user0001");
        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let start = Scheduled::AtHeight(app.block_info().height + 10);
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(Expiration::AtHeight(app.block_info().height + 200)),
            start: Some(start),
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );


        // Claim some tokens before stage starts
        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };
       let error = app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap_err();

        assert_eq!(
            error.downcast::<ContractError>().unwrap(),
            ContractError::StageNotBegun { stage: 1u8, start }
        );

        app.update_block(jump_100_blocks);

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };

        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query balance of user1 after stage starts
        let balance_response = query_balance_native(
            &mut app,
            &user1,
            "ujunox".to_string(),
        );

        assert_eq!(balance_response, Coin { denom: "ujunox".to_string(), amount: Uint128::from(100_000_000u128) });
    }

    #[test]
    fn stage_expiration() {
        let admin = admin();
        let user1 = user("user0001");
        let user2 = user("user0002");
        let mut app = mock_app();

        let rev_share_contract_addr = initialize_rev_share_contract(&mut app);

        // wait for 1 block
        app.update_block(next_block);

        // create new stage
        let expiration = Expiration::AtHeight(app.block_info().height + 10);
        let msg = ExecuteMsg::CreateNewStage {
            total_amount: Uint128::from(200_000_000u128),
            snapshot_block: Some(app.block_info().height),
            expiration: Some(expiration),
            start: None,
        };

        app.execute_contract(
            admin.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        mint_native(
            &mut app, rev_share_contract_addr.clone().to_string(),
            "ujunox".to_string(), Uint128::from(200_000_000u128),
        );

        // Claim some tokens before stage expires

        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };

        app.execute_contract(
            user1.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap();

        // query native balance of user1

        let balance_response = query_balance_native(
            &mut app,
            &user1,
            "ujunox".to_string(),
        );

        assert_eq!(balance_response, Coin { denom: "ujunox".to_string(), amount: Uint128::from(100_000_000u128) });

        // move 100 blocks forward

        app.update_block(jump_100_blocks);

        // Claim some tokens after stage expires

        let msg = ExecuteMsg::Claim {
            stage: 1u8,
        };

        let error = app.execute_contract(
            user2.clone(),
            rev_share_contract_addr.clone(),
            &msg,
            &[],
        ).unwrap_err();

        assert_eq!(
            error.downcast::<ContractError>().unwrap(),
            ContractError::StageExpired { stage: 1u8, expiration }
        );
    }
}


