#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Addr, attr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult, Storage, Timestamp, to_binary, Uint128, Uint64, WasmMsg};
use cw20::Cw20ExecuteMsg;
use cw2::set_contract_version;

use crate::{msg::{ExecuteMsg, InstantiateMsg, OwnerAddressResponse, QueryMsg, TokenAddressResponse, VestingAccountResponse, VestingData}, state::{Account, ACCOUNTS, OWNER_ADDRESS, TOKEN_ADDRESS}};
use crate::state::{BLOCK_TIME, compute_payments_for_time_interval, Payment, PaymentStatus};

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

    let block_time = msg.block_time;
    BLOCK_TIME.save(deps.storage, &block_time)?;

    Ok(Response::new().add_attribute("owner_address", owner_address).add_attribute("token_address", token_address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateOwnerAddress { address } => {
            update_owner_address(deps, env, info, address)
        }
        ExecuteMsg::UpdateBlockTime {
            block_time,
        } => update_block_time(deps, env, info, block_time),
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
        } => register_vesting_account(deps, env, info, address, start_time, end_time, vesting_amount),
        ExecuteMsg::Claim { recipient } => claim(deps, env, info, recipient),
    }
}

fn only_owner(storage: &dyn Storage, sender: Addr) -> StdResult<()> {
    if OWNER_ADDRESS.load(storage)? != sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    Ok(())
}

fn update_owner_address(deps: DepsMut, _env: Env, info: MessageInfo, owner_address: Addr) -> StdResult<Response> {
    only_owner(deps.storage, info.sender)?;

    OWNER_ADDRESS.save(deps.storage, &owner_address)?;

    Ok(Response::new().add_attribute("action", "update_owner_address").add_attribute("owner_address", &owner_address))
}

fn update_block_time(deps: DepsMut, _env: Env, info: MessageInfo, block_time: Uint64) -> StdResult<Response> {
    only_owner(deps.storage, info.sender)?;

    let accounts_addr: Vec<Addr> = ACCOUNTS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|item| item.map(Into::into))
        .collect::<StdResult<_>>()?;

    let mut all_accounts: Vec<Account> = accounts_addr.iter().map(|addr| ACCOUNTS.load(deps.storage, addr)).collect::<StdResult<_>>()?;

    for account in all_accounts.iter_mut() {
        let payments = &account.scheduled_payments;
        let mut new_payments: Vec<Payment> = Vec::new();
        for payment in payments {
            if payment.status != PaymentStatus::Pending || payment.height.u64() < _env.block.height {
                new_payments.push(payment.clone());
            }
        }
        let vested_amount = account.vested_amount(&_env.block, None)?;
        let new_vesting_amount = account.vesting_amount.checked_sub(vested_amount)?;
        let future_payments = compute_payments_for_time_interval(
            block_time,
            &_env.block,
            _env.block.time.clone(),
            account.end_time.clone(),
            new_vesting_amount,
        );
        new_payments.extend(future_payments);
        account.scheduled_payments = new_payments;
    }

    for account in all_accounts.iter() {
        ACCOUNTS.save(deps.storage, &account.address, account)?;
    }

    BLOCK_TIME.save(deps.storage, &block_time)?;

    Ok(Response::new()
        .add_attribute("action", "update_block_time")
        .add_attribute("block_time", block_time)
    )
}

fn register_vesting_account(deps: DepsMut, env: Env, _info: MessageInfo, address: Addr, start_time: Timestamp, end_time: Timestamp, vesting_amount: Uint128) -> StdResult<Response> {
    // vesting_account existence check
    if ACCOUNTS.has(deps.storage, &address) {
        return Err(StdError::generic_err("already exists"));
    }

    let block_time = BLOCK_TIME.load(deps.storage)?;
    let payments = compute_payments_for_time_interval(block_time, &env.block, start_time, end_time, vesting_amount);

    let account = Account {
        address: address.clone(),
        vesting_amount: vesting_amount.clone(),
        start_time: start_time,
        end_time: end_time,
        scheduled_payments: payments,
    };
    account.validate(&env.block)?;

    ACCOUNTS.save(
        deps.storage,
        &address,
        &account,
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

    let mut account = account.unwrap();
    let updated_payments = account.scheduled_payments.iter().map(|payment| {
        match payment.status {
            PaymentStatus::Pending => Payment {
                height: payment.height,
                amount: payment.amount,
                timestamp: payment.timestamp,
                status: PaymentStatus::Revoked,
            },
            PaymentStatus::Paid => payment.clone(),
            PaymentStatus::Revoked => payment.clone()
        }
    }).collect();

    account.scheduled_payments = updated_payments;

    ACCOUNTS.save(
        deps.storage,
        &address,
        &account,
    )?;

    let vested_amount = account
        .vested_amount(&env.block, None)?;
    let claimed_amount = account.claimed_amount(&env.block, None)?;

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
        None => info.sender,
        Some(addr) => addr,
    };
    let token_address = TOKEN_ADDRESS.load(deps.storage)?;

    let account = ACCOUNTS.may_load(deps.storage, &_recipient)?;
    if account.is_none() {
        return Err(StdError::generic_err("vesting entry is not found"));
    }

    let mut account = account.unwrap();


    let claimable_amount = account.claimable_amount(&env.block, None)?;

    let updated_payments = account.scheduled_payments.iter().map(|payment| {
        match payment.status {
            PaymentStatus::Pending => {
                if payment.height.u64() <= env.block.height {
                    Payment {
                        height: payment.height,
                        amount: payment.amount,
                        timestamp: payment.timestamp,
                        status: PaymentStatus::Paid,
                    }
                } else {
                    payment.clone()
                }
            }
            PaymentStatus::Paid => payment.clone(),
            PaymentStatus::Revoked => payment.clone()
        }
    }).collect();
    account.scheduled_payments = updated_payments;
    ACCOUNTS.save(deps.storage, &_recipient, &account)?;

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
            with_payments,
        } => to_binary(&query_vesting_account(deps, env, address, height, with_payments)?),
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

fn query_vesting_account(deps: Deps, env: Env, address: Addr, height: Option<u64>, with_payments: Option<bool>) -> StdResult<VestingAccountResponse> {
    let account = ACCOUNTS.load(deps.storage, &address)?;


    let vested_amount = account.vested_amount(&env.block, height)?;
    let claimed_amount = account.claimed_amount(&env.block, height)?;

    let claimable_amount = account.claimable_amount(&env.block, height)?;

    let vesting_data = VestingData {
        vesting_amount: account.vesting_amount,
        vested_amount,
        start_time: account.start_time,
        end_time: account.end_time,
        claimable_amount,
        claimed_amount,
        scheduled_payments: if with_payments.unwrap_or(false) {
            Some(account.scheduled_payments)
        } else {
            None
        },
    };

    Ok(VestingAccountResponse { address, vestings: vesting_data })
}

#[cfg(test)]
mod testing {
    use cosmwasm_std::{Addr, from_binary, testing::{mock_dependencies, mock_env, mock_info}};

    use crate::msg::InstantiateMsg;

    use super::*;

    const TESTING_BLOCK_TIME: u64 = 5000u64;
    const INITIAL_TIMESTAMP: Timestamp = Timestamp::from_nanos(1665157155000000000);

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
            block_time: Uint64::new(TESTING_BLOCK_TIME),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::OwnerAddress {}).unwrap();
        let owner_response: OwnerAddressResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("addr0001".to_string()), owner_response.owner_address);

        let res = query(deps.as_ref(), env.clone(), QueryMsg::TokenAddress {}).unwrap();
        let token_response: TokenAddressResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("token0001".to_string()), token_response.token_address);
    }

    #[test]
    fn register_vesting_account() {
        let mut deps = mock_dependencies();

        let initial_block = 0u64;

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
            block_time: Uint64::new(TESTING_BLOCK_TIME),
        };

        let mut env = mock_env();
        env.block.time = INITIAL_TIMESTAMP;
        env.block.height = initial_block;
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(120_000u32),
            start_time: INITIAL_TIMESTAMP.plus_seconds(5),
            end_time: INITIAL_TIMESTAMP.plus_seconds(600),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = INITIAL_TIMESTAMP.plus_seconds(5);
        env.block.height += 1;

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            height: None,
            with_payments: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                vesting_amount: Uint128::from(120_000u32),
                vested_amount: Uint128::from(1008u32),
                claimable_amount: Uint128::from(1008u32),
                claimed_amount: Uint128::zero(),
                start_time: INITIAL_TIMESTAMP.plus_seconds(5),
                end_time: INITIAL_TIMESTAMP.plus_seconds(600),
                scheduled_payments: None,
            },
        })
    }

    #[test]
    fn claim() {
        let mut deps = mock_dependencies();
        let initial_block = 0u64;

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
            block_time: Uint64::new(TESTING_BLOCK_TIME),
        };

        let mut env = mock_env();
        env.block.time = INITIAL_TIMESTAMP;
        env.block.height = initial_block;
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(120_000u32),
            start_time: INITIAL_TIMESTAMP.plus_seconds(5),
            end_time: INITIAL_TIMESTAMP.plus_seconds(600),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = INITIAL_TIMESTAMP.plus_seconds(5);
        env.block.height += 1;
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            height: None,
            with_payments: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                vesting_amount: Uint128::new(120_000u128),
                vested_amount: Uint128::new(1008u128),
                claimable_amount: Uint128::new(1008u128),
                claimed_amount: Uint128::zero(),
                start_time: INITIAL_TIMESTAMP.plus_seconds(5),
                end_time: INITIAL_TIMESTAMP.plus_seconds(600),
                scheduled_payments: None,
            },
        });

        let msg = ExecuteMsg::Claim {
            recipient: None,
        };
        let info = mock_info("addr0002", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            height: None,
            with_payments: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                vesting_amount: Uint128::new(120_000u128),
                vested_amount: Uint128::new(1008u128),
                claimable_amount: Uint128::new(0u128),
                claimed_amount: Uint128::new(1008u128),
                start_time: INITIAL_TIMESTAMP.plus_seconds(5),
                end_time: INITIAL_TIMESTAMP.plus_seconds(600),
                scheduled_payments: None,
            },
        })
    }

    #[test]
    fn update_block_time() {
        let mut deps = mock_dependencies();
        let initial_block = 0u64;

        let msg = InstantiateMsg {
            token_address: Addr::unchecked("token0001".to_string()),
            owner_address: Some(Addr::unchecked("addr0001".to_string())),
            block_time: Uint64::new(TESTING_BLOCK_TIME),
        };

        let mut env = mock_env();
        env.block.time = INITIAL_TIMESTAMP;
        env.block.height = initial_block;
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            vesting_amount: Uint128::from(100_000u32),
            start_time: INITIAL_TIMESTAMP.plus_seconds(5),
            end_time: INITIAL_TIMESTAMP.plus_seconds(105),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = INITIAL_TIMESTAMP.plus_seconds(10);
        env.block.height += 2;
        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            height: None,
            with_payments: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                vesting_amount: Uint128::new(100_000u128),
                vested_amount: Uint128::new(10_000u128),
                claimable_amount: Uint128::new(10_000u128),
                claimed_amount: Uint128::zero(),
                start_time: INITIAL_TIMESTAMP.plus_seconds(5),
                end_time: INITIAL_TIMESTAMP.plus_seconds(105),
                scheduled_payments: None,
            },
        });

        let msg = ExecuteMsg::UpdateBlockTime {
            block_time: Uint64::new(10_000),
        };
        let info = mock_info("addr0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::VestingAccount {
            address: Addr::unchecked("addr0002".to_string()),
            height: None,
            with_payments: None,
        }).unwrap();
        let vesting_response: VestingAccountResponse = from_binary(&res).unwrap();

        assert_eq!(vesting_response, VestingAccountResponse {
            address: Addr::unchecked("addr0002".to_string()),
            vestings: VestingData {
                vesting_amount: Uint128::new(100_000u128),
                vested_amount: Uint128::new(15_000u128),
                claimable_amount: Uint128::new(15_000u128),
                claimed_amount: Uint128::zero(),
                start_time: INITIAL_TIMESTAMP.plus_seconds(5),
                end_time: INITIAL_TIMESTAMP.plus_seconds(105),
                scheduled_payments: None,
            },
        });

    }
}
