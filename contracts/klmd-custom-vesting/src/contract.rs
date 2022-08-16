#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{DepsMut, Env, MessageInfo, StdResult, Response, Addr, StdError, Storage, Timestamp, Uint128, WasmMsg, to_binary, attr, Binary, Deps};
use cw20::Cw20ExecuteMsg;

use crate::{msg::{InstantiateMsg, ExecuteMsg, QueryMsg, OwnerAddressResponse, VestingAccountResponse, VestingData, TokenAddressResponse}, state::{OWNER_ADDRESS, TOKEN_ADDRESS, ACCOUNTS, Account}};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let owner_address = msg.owner_address.unwrap_or(info.sender);
    OWNER_ADDRESS.save(deps.storage, &owner_address)?;

    let token_address = msg.token_address;
    TOKEN_ADDRESS.save(deps.storage, &token_address)?;
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
        } =>  register_vesting_account(deps, env, info, address, start_time, end_time, vesting_amount),
        ExecuteMsg::Claim {recipient} => claim(deps, env, info, recipient),
    }
}

fn only_owner(storage: &dyn Storage, sender: Addr) -> StdResult<()> {
    if OWNER_ADDRESS.load(storage)? != sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    Ok(())
}

fn update_owner_address(deps: DepsMut, _env: Env, info: MessageInfo, owner_address: Addr) -> StdResult<Response>  {
    only_owner(deps.storage, info.sender)?;

    OWNER_ADDRESS.save(deps.storage, &owner_address)?;

    Ok(Response::new().add_attribute("action", "update_owner_address").add_attribute("owner_address", &owner_address))
}

fn register_vesting_account(deps: DepsMut, _env: Env, _info: MessageInfo, address: Addr, start_time: Timestamp, end_time: Timestamp, vesting_amount: Uint128) -> StdResult<Response> {
    // vesting_account existence check
    if ACCOUNTS.has(deps.storage, &address) {
        return Err(StdError::generic_err("already exists"));
    }

    ACCOUNTS.save(
        deps.storage,
        &address,
        &Account {
            address: address.clone(),
            vesting_amount: vesting_amount.clone(),
            start_time: start_time,
            end_time: end_time,
            claimed_amount: Uint128::zero(),
        }
    )?;

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
    let _recipient = recipient.unwrap_or(info.sender);
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
        } => to_binary(&query_vesting_account(deps, env, address)?),
        QueryMsg::TokenAddress {} => to_binary(&query_token_address(deps, env)?),
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

fn query_vesting_account(deps: Deps, env: Env, address: Addr) -> StdResult<VestingAccountResponse> {
    let account = ACCOUNTS.load(deps.storage, &address)?;

    let vested_amount = account.vested_amount(&env.block)?;
    let claimed_amount = account.claimed_amount;

    let claimable_amount = vested_amount.checked_sub(claimed_amount)?;

    let vesting_data = VestingData {
        vesting_amount: account.vesting_amount,
        vested_amount,
        start_time: account.start_time,
        end_time: account.end_time,
        claimable_amount: claimable_amount,
        claimed_amount: account.claimed_amount,
    };

    Ok(VestingAccountResponse { address, vestings: vesting_data })
}

#[cfg(test)]
mod testing {
    use super::*;
    use cosmwasm_std::{testing::{mock_dependencies, mock_env, mock_info}, Addr, from_binary};

    use crate::msg::InstantiateMsg;

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
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();


        let mut deps = mock_dependencies();
        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100u32),
            start_time: Timestamp::from_nanos(0),
            end_time: Timestamp::from_nanos(100),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = Timestamp::from_nanos(200);
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData { 
                vesting_amount: Uint128::from(100u32), 
                vested_amount: Uint128::from(100u32), 
                claimable_amount: Uint128::from(100u32),
                claimed_amount: Uint128::zero(),
                start_time: Timestamp::from_nanos(0), 
                end_time: Timestamp::from_nanos(100), 
            },
        })
    }

    fn claim() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
        };

        let mut env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();


        let mut deps = mock_dependencies();
        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100u32),
            start_time: Timestamp::from_nanos(0),
            end_time: Timestamp::from_nanos(100),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = Timestamp::from_nanos(200);
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData { 
                vesting_amount: Uint128::from(100u32), 
                vested_amount: Uint128::from(100u32), 
                claimable_amount: Uint128::from(100u32), 
                claimed_amount: Uint128::zero(),
                start_time: Timestamp::from_nanos(0), 
                end_time: Timestamp::from_nanos(100), 
            },
        });

        let msg = ExecuteMsg::Claim {
            recipient: None,
        };
        let info = mock_info("addr0000", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData { 
                vesting_amount: Uint128::from(100u32), 
                vested_amount: Uint128::from(100u32), 
                claimable_amount: Uint128::from(0u32),
                claimed_amount: Uint128::from(100u32), 
                start_time: Timestamp::from_nanos(0), 
                end_time: Timestamp::from_nanos(100), 
            },
        })
    }
}