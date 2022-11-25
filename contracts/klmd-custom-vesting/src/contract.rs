use cosmwasm_std::{Addr, attr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult, Storage, Timestamp, to_binary, Uint128, Uint64, WasmMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw20::Cw20ExecuteMsg;
use cw20_base::ContractError;
use cw2::set_contract_version;

use crate::{msg::{ExecuteMsg, InstantiateMsg, OwnerAddressResponse, QueryMsg, TokenAddressResponse, VestingAccountResponse, VestingTotalResponse}, state::{Account, ACCOUNTS, get_vesting_data_from_account, OWNER_ADDRESS, TOKEN_ADDRESS, TotalVestingInfo, VESTING_DATA, VESTING_TOTAL, VestingData}};
use crate::msg::{InfoResponse, MigrateMsg};

pub(crate) const CONTRACT_NAME: &str = "crates.io:klmd-custom-vesting";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let owner_address = msg.owner_address.unwrap_or(info.sender);
    OWNER_ADDRESS.save(deps.storage, &owner_address)?;

    let token_address = msg.token_address;
    TOKEN_ADDRESS.save(deps.storage, &token_address)?;
    let vesting_info = TotalVestingInfo {
        prevesting_amount: Uint128::zero(),
        prevested_amount: Uint128::zero(),
        vesting_amount: Uint128::zero(),
        vested_amount: Uint128::zero(),
        claimed_amount: Uint128::zero(),
    };
    VESTING_TOTAL.save(deps.storage, &vesting_info, _env.block.height)?;
    Ok(Response::new().add_attribute("owner_address", owner_address).add_attribute("token_address", token_address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateOwnerAddress { address } => {
            update_owner_address(deps, env, info, address)
        },
        ExecuteMsg::DeregisterVestingAccount {
            address,
            vested_token_recipient,
            left_vesting_token_recipient
        } => deregister_vesting_account(deps, env, info, address, vested_token_recipient, left_vesting_token_recipient),
        ExecuteMsg::RegisterVestingAccount {
            address,
            start_time,
            end_time,
            vesting_amount,
            prevesting_amount,
        } =>  register_vesting_account(deps, env, info, address, start_time, end_time, vesting_amount, prevesting_amount),
        ExecuteMsg::Claim {recipient} => claim(deps, env, info, recipient),
        ExecuteMsg::Snapshot {} => snapshot(deps, env, info),
        ExecuteMsg::ProposalHook(_) => snapshot(deps, env, info),
    }
}

fn only_owner(storage: &dyn Storage, sender: Addr) -> StdResult<()> {
    if OWNER_ADDRESS.load(storage)? != sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    Ok(())
}

fn snapshot(deps: DepsMut, _env: Env, _info: MessageInfo) -> StdResult<Response> {
    let accounts_addr: Vec<Addr> = ACCOUNTS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|item| item.map(Into::into))
        .collect::<StdResult<_>>()?;

    let account_vest_data: Vec<(Addr, VestingData)> = accounts_addr
        .into_iter()
        .map(| addr | ACCOUNTS.load(deps.storage, &addr).unwrap())
        .map(| account| (account.clone().address, get_vesting_data_from_account(account.clone(), &_env.block).unwrap()))
        .collect();

    let height = _env.block.clone().height - 1;

    for (addr, account_data) in account_vest_data.iter() {
        VESTING_DATA.save(deps.storage, &addr, &account_data, height)?;
    }
    let (_, data): (Vec<Addr>, Vec<VestingData>) = account_vest_data.into_iter().unzip();
    let total_vesting_info = compute_total_vesting_info(data)?;

    VESTING_TOTAL.save(deps.storage, &total_vesting_info, height)?;


    Ok(Response::new().add_attribute("action", "snapshot").add_attribute("height", Uint64::new(_env.block.height)))
}

fn update_owner_address(deps: DepsMut, _env: Env, info: MessageInfo, owner_address: Addr) -> StdResult<Response>  {
    only_owner(deps.storage, info.sender)?;

    OWNER_ADDRESS.save(deps.storage, &owner_address)?;

    Ok(Response::new().add_attribute("action", "update_owner_address").add_attribute("owner_address", &owner_address))
}

fn compute_total_vesting_info(account_vesting_data: Vec<VestingData>) -> StdResult<TotalVestingInfo> {
    let total_prevesting= Uint128::zero();
    let mut total_prevested = Uint128::zero();
    let mut total_vested = Uint128::zero();
    let mut total_claimed= Uint128::zero();
    let mut total_vesting= Uint128::zero();
    for account_data in account_vesting_data.into_iter() {
        total_prevested += account_data.prevested_amount;
        total_vested += account_data.vested_amount;
        total_claimed += account_data.claimed_amount;

        if account_data.vesting_amount > account_data.prevested_amount {
            total_vesting += account_data.prevested_amount;
        } else {
            total_vesting += account_data.vesting_amount;
        }
    }

    Ok(
        TotalVestingInfo {
            prevesting_amount: total_prevesting,
            prevested_amount: total_prevested,
            vesting_amount: total_vesting,
            vested_amount: total_vested,
            claimed_amount: total_claimed,
        }
    )
}

fn register_vesting_account(deps: DepsMut, env: Env, _info: MessageInfo, address: Addr, start_time: Timestamp, end_time: Timestamp, vesting_amount: Uint128, prevesting_amount: Uint128) -> StdResult<Response> {
    // vesting_account existence check
    if ACCOUNTS.has(deps.storage, &address) {
        return Err(StdError::generic_err("already exists"));
    }

    let account = Account {
        address: address.clone(),
        prevesting_amount: prevesting_amount.clone(),
        vesting_amount: vesting_amount.clone(),
        registration_time: env.block.time,
        start_time: start_time,
        end_time: end_time,
        claimed_amount: Uint128::zero(),
    };
    account.validate(&env.block)?;

    ACCOUNTS.save(
        deps.storage,
        &address,
        &account,
    )?;

    let _ = snapshot(deps, env, _info)?;

    Ok(Response::new()
        .add_attribute("action", "register_vesting_account")
        .add_attribute("address", &address)
        .add_attribute("vesting_amount", vesting_amount.clone())
    )
}

fn deregister_vesting_account(deps: DepsMut, env: Env, info: MessageInfo, address: Addr, vested_token_recipient: Option<Addr>, left_vesting_token_recipient: Option<Addr>) -> StdResult<Response> {
    only_owner(deps.storage, info.sender.clone())?;
    let token_address = TOKEN_ADDRESS.load(deps.storage)?;
    let mut messages: Vec<WasmMsg> = vec![];

    // vesting_account existence check
    let account = ACCOUNTS.may_load(deps.storage, &address)?;
    if account.is_none() {
        return Err(StdError::generic_err("vesting entry is not found"));
    }

    let account = account.unwrap();

    // remove vesting account
    ACCOUNTS.remove(deps.storage, &address);

    let _ = snapshot(deps, env.clone(), info.clone())?; // save current snapshot
    //VESTING_DATA.remove(deps.storage, &address, env.block.height)?;

    let vested_amount = account
        .vested_amount(&env.block)?;
    let claimed_amount = account.claimed_amount;

    let claimable_amount = vested_amount.checked_sub(claimed_amount)?;
    if !claimable_amount.is_zero() {
        let _recipient = vested_token_recipient.unwrap_or(account.address);
        let claimable_message = WasmMsg::Execute {
                    contract_addr: token_address.to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: _recipient.to_string(),
                        amount: claimable_amount,
                    })?,
        };
        messages.push(claimable_message);
    }

    // transfer left vesting amount to owner or
    // the given `left_vesting_token_recipient` address
    let left_vesting_amount = account.vesting_amount.checked_sub(vested_amount)?;
    if !left_vesting_amount.is_zero() {
        let _recipient = left_vesting_token_recipient.unwrap_or(info.sender.clone());
        let left_vesting_message = WasmMsg::Execute {
            contract_addr: token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: _recipient.to_string(),
                amount: left_vesting_amount,
            })?,
        };
        messages.push(left_vesting_message);
    }

    Ok(
        Response::new()
            .add_messages(messages)
            .add_attribute("action", "deregister_vesting_account")
            .add_attribute("address", address)
            .add_attribute("vesting_amount", account.vesting_amount)
            .add_attribute("claimable_amount", claimable_amount)
            .add_attribute("vested_amount", vested_amount)
            .add_attribute("left_vesting_amount", left_vesting_amount)
    )
}

fn claim(deps: DepsMut, env: Env, info: MessageInfo, recipient: Option<Addr>) -> StdResult<Response> {
    let _recipient = match recipient {
        None => info.clone().sender,
        Some(addr) => addr,
    };
    let token_address = TOKEN_ADDRESS.load(deps.storage)?;

    let account = ACCOUNTS.may_load(deps.storage, &_recipient)?;
    if account.is_none() {
        return Err(StdError::generic_err("vesting entry is not found"));
    }

    let mut account = account.unwrap();
    let vested_amount = account.vested_amount(&env.block)?;
    let claimed_amount = account.claimed_amount;

    let claimable_amount = vested_amount.checked_sub(claimed_amount)?;

    account.claimed_amount = vested_amount;
    if account.claimed_amount == account.vesting_amount {
        ACCOUNTS.remove(deps.storage, &_recipient);
    } else {
        ACCOUNTS.save(deps.storage, &_recipient, &account)?;
    }
    let _ = snapshot(deps, env.clone(), info.clone())?;

    let res = Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: _recipient.to_string(),
                amount: claimable_amount,
            })?,
        })
        .add_attributes(vec![
            attr("action", "claim"),
            attr("address", _recipient),
            attr("amount", claimable_amount),
        ]);
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::OwnerAddress {} => to_binary(&query_owner_address(deps, env)?),
        QueryMsg::VestingAccount {
            address,
            height,
        } => to_binary(&query_vesting_account(deps, env, address, height)?),
        QueryMsg::TokenAddress {} => to_binary(&query_token_address(deps, env)?),
        QueryMsg::VestingTotal { height } => to_binary(&query_vesting_total(deps, env, height)?),
        QueryMsg::Info {} => to_binary(&query_info(deps)?),
    }
}

fn query_owner_address(deps: Deps, _env: Env) -> StdResult<OwnerAddressResponse> {
    let owner_address = OWNER_ADDRESS.load(deps.storage)?;
    Ok(OwnerAddressResponse {
        owner_address,
    })
}

fn query_token_address(deps: Deps, _env: Env) -> StdResult<TokenAddressResponse> {
    let token_address = TOKEN_ADDRESS.load(deps.storage)?;
    Ok(TokenAddressResponse {
        token_address,
    })
}

fn query_vesting_total(deps: Deps, env: Env, height: Option<u64>) -> StdResult<VestingTotalResponse> {
    let input_height = height.unwrap_or(env.block.height);
    let total_vesting_info = VESTING_TOTAL.may_load_at_height(deps.storage, input_height)?.unwrap_or_default();

    Ok(VestingTotalResponse {
        info: total_vesting_info,
    })
}

fn query_vesting_account(deps: Deps, env: Env, address: Addr, height: Option<u64>) -> StdResult<VestingAccountResponse> {
    let input_height = height.unwrap_or(env.block.height);
    let vesting_data = VESTING_DATA.may_load_at_height(deps.storage, &address, input_height)?.unwrap_or_default();

    Ok(VestingAccountResponse { address, vestings: vesting_data })
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let info = cw2::get_contract_version(deps.storage)?;
    Ok(InfoResponse { info })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Set contract to version to latest
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg(test)]
mod testing {
    use cosmwasm_std::{Addr, from_binary, testing::{mock_dependencies, mock_env, mock_info}};

    use crate::msg::InstantiateMsg;

    use super::*;

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::OwnerAddress{}).unwrap();
        let owner_response: OwnerAddressResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("addr0001".to_string()), owner_response.owner_address);

        let res = query(deps.as_ref(), env.clone(), QueryMsg::TokenAddress{}).unwrap();
        let token_response: TokenAddressResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("token0001".to_string()), token_response.token_address);
    }

    #[test]
    fn register_vesting_account() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
        };

        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(0);
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();


        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100u32),
            prevesting_amount: Uint128::from(10u32),
            start_time: Timestamp::from_nanos(100),
            end_time: Timestamp::from_nanos(200),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        env.block.height += 20;
        env.block.time = Timestamp::from_nanos(50);
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(10u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(0u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        env.block.height += 1;
        env.block.time = Timestamp::from_nanos(105);
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), ExecuteMsg::Snapshot{}).unwrap();

        env.block.height += 1;
        env.block.time = Timestamp::from_nanos(110);
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(100u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(5u32),
                claimable_amount: Uint128::from(5u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        })
    }

    #[test]
    fn claim() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
        };

        let mut env = mock_env();
        // ##### TIME 0 #####
        env.block.time = Timestamp::from_nanos(0);
        env.block.height = 1000;
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100u32),
            prevesting_amount: Uint128::from(10u32),
            start_time: Timestamp::from_nanos(100),
            end_time: Timestamp::from_nanos(200),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // ##### TIME 1 ##### (10 seconds before start_time)
        env.block.time = Timestamp::from_nanos(90);
        env.block.height += 1; // 1001
        let msg = ExecuteMsg::Snapshot {};
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(90u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(0u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // ##### TIME 2 ##### (10 seconds after start_time)
        env.block.time = Timestamp::from_nanos(110);
        env.block.height += 1; // 1002

        let msg = ExecuteMsg::Claim {
            recipient: None,
        };
        let info = mock_info("addr0002", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(100u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(10u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::from(10u32),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // ##### TIME 3 ##### (15 seconds after start_time without snapshot)
        env.block.time = Timestamp::from_nanos(115);
        env.block.height += 1; // 1003
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();


        // it should be freezed since there were no snapshot in the middle
        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(100u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(10u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::from(10u32),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // check old data for TIME 1
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: Some(1001),
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(
            vesting_response,
            VestingAccountResponse {
                address: Addr::unchecked("addr0002".to_string()),
                vestings: VestingData {
                    prevesting_amount: Uint128::from(10u32),
                    prevested_amount: Uint128::from(90u32),
                    vesting_amount: Uint128::from(100u32),
                    vested_amount: Uint128::from(0u32),
                    claimable_amount: Uint128::from(0u32),
                    claimed_amount: Uint128::from(0u32),
                    registration_time: Timestamp::from_nanos(0),
                    start_time: Timestamp::from_nanos(100),
                    end_time: Timestamp::from_nanos(200),
                },
            }
        );
    }

    #[test]
    fn test_snapshot() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
        };

        let mut env = mock_env();
        // ##### TIME 0 #####
        env.block.time = Timestamp::from_nanos(0);
        env.block.height = 1000;
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100u32),
            prevesting_amount: Uint128::from(10u32),
            start_time: Timestamp::from_nanos(100),
            end_time: Timestamp::from_nanos(200),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(10u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(0u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // ##### TIME 1 ##### (20 seconds after start_time)
        env.block.time = Timestamp::from_nanos(80);
        env.block.height += 1; // 1001

        // first snapshot
        let msg = ExecuteMsg::Snapshot {};
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(80u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(0u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // ##### TIME 2 ##### (10 seconds after start_time)
        env.block.time = Timestamp::from_nanos(110);
        env.block.height += 1; // 1002

        // second snapshot
        let msg = ExecuteMsg::Snapshot {};
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: None,
        }).unwrap();

        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(100u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(10u32),
                claimable_amount: Uint128::from(10u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });

        // query for snapshot at height 1001
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()), height: Some(1001),
        }).unwrap();

        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::from(10u32),
                prevested_amount: Uint128::from(80u32),
                vesting_amount: Uint128::from(100u32),
                vested_amount: Uint128::from(0u32),
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::zero(),
                registration_time: Timestamp::from_nanos(0),
                start_time: Timestamp::from_nanos(100),
                end_time: Timestamp::from_nanos(200),
            },
        });
    }

    #[test]
    pub fn test_migrate_update_version() {
        let mut deps = mock_dependencies();
        cw2::set_contract_version(&mut deps.storage, "my-contract", "old-version").unwrap();
        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
        let version = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(version.version, CONTRACT_VERSION);
        assert_eq!(version.contract, CONTRACT_NAME);
    }
}
