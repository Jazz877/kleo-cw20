#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, WasmMsg, Storage, StdError, Addr};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetOwnerResponse, InstantiateMsg, Payment, PaymentsResponse, QueryMsg};
use crate::state::{next_id, PaymentState, PAYMENTS, OWNER_ADDRESS, TOKEN_ADDRESS};
use cw20::Cw20ExecuteMsg;

fn only_owner(storage: &dyn Storage, sender: Addr) -> StdResult<()> {
    if OWNER_ADDRESS.load(storage)? != sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let owner_address = if msg.owner_address.is_none() {
        _info.sender
    } else {
        msg.owner_address.unwrap()
    };
    let token_address = msg.token_address;

    OWNER_ADDRESS.save(deps.storage, &owner_address)?;
    TOKEN_ADDRESS.save(deps.storage, &token_address)?;

    for p in msg.schedule.into_iter() {
        let id = next_id(deps.storage)?;
        PAYMENTS.save(
            deps.storage,
            id.into(),
            &PaymentState {
                payment: p,
                paid: false,
                id,
            },
        )?;
    }
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", owner_address)
    )
    //.add_attribute("count", msg.schedule))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Pay {} => execute_pay(deps, env),
        ExecuteMsg::RevokePayments { payment_ids } => revoke_payments(deps, _info, payment_ids),
    }
}

pub fn execute_pay(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let to_be_paid: Vec<PaymentState> = PAYMENTS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|r| match r {
            Ok(r) => Some(r.1),
            _ => None,
        })
        .filter(|p| !p.paid && p.payment.time.is_expired(&env.block))
        .collect();

    let token_address = TOKEN_ADDRESS.load(deps.storage)?;

    // Get cosmos payment messages
    let payment_msgs: Vec<CosmosMsg> = to_be_paid
        .clone()
        .into_iter()
        .map(|p| get_payment_message(&p.payment, token_address.clone()))
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    // Update payments to paid
    for p in to_be_paid.into_iter() {
        PAYMENTS.update(deps.storage, p.id.into(), |p| match p {
            Some(p) => Ok(PaymentState { paid: true, ..p }),
            None => Err(ContractError::PaymentNotFound {}),
        })?;
    }

    Ok(Response::new().add_messages(payment_msgs))
    //.add_attribute("paid", to_be_paid))
}

pub fn revoke_payments(deps: DepsMut, info: MessageInfo, payment_ids: Vec<u64>) -> Result<Response, ContractError> {
    let is_owner = only_owner(deps.storage, info.sender);
    match is_owner {
        Ok(..) => {
            let to_delete: Vec<u64> = payment_ids.into_iter()
                .filter(|el| {
                    let payment = PAYMENTS.may_load(deps.storage, *el);
                    match payment {
                        Ok(el) => {
                            match el {
                                Some(_el) => !_el.paid,
                                None => false,
                            }
                        }
                        Err(..) => false,
                    }
                }).collect();
            for el in to_delete.clone().into_iter() {
                PAYMENTS.remove(deps.storage, el);
            }
            Ok(Response::default().add_attribute("removed_ids", format!("{:?}", to_delete.clone())))
        }
        Err(..) => Err(ContractError::Unauthorized {})
    }
}

pub fn get_payment_message(p: &Payment, token_address: Addr) -> StdResult<CosmosMsg> {
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: p.recipient.to_string(),
        amount: p.amount,
    };

    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_address.to_string(),
        msg: to_binary(&transfer_cw20_msg)?,
        funds: vec![],
    };

    Ok(exec_cw20_transfer.into())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetPayments {} => to_binary(&query_payments(deps)),
        QueryMsg::GetOwner {} => to_binary(&query_owner(deps)),
    }
}

fn query_payments(deps: Deps) -> PaymentsResponse {
    PaymentsResponse {
        payments: PAYMENTS
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(|p| match p {
                Ok(p) => Some(p.1),
                Err(_) => None,
            })
            .collect(),
    }
}

fn query_owner(deps: Deps) -> GetOwnerResponse {
    let owner = OWNER_ADDRESS.load(deps.storage);
    GetOwnerResponse {
        owner: owner.unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, from_binary, Uint128};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw_utils::Expiration;
    use crate::contract::{execute, instantiate, query};
    use crate::ContractError;
    use crate::msg::{ExecuteMsg, GetOwnerResponse, InstantiateMsg, Payment, PaymentsResponse, QueryMsg};
    use crate::state::PaymentState;

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            schedule: vec![
                Payment {
                    recipient: Addr::unchecked("addr0002"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(10u64),
                }
            ],
            owner_address: Some(Addr::unchecked("owner0001".to_string())),
            token_address: Addr::unchecked("kleo00001"),
        };

        let mut env = mock_env();
        let info = mock_info("owner0001", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // it worked, let's query the state
        let res = query(deps.as_ref(), env.clone(), QueryMsg::GetOwner {}).unwrap();
        let response: GetOwnerResponse = from_binary(&res).unwrap();
        assert_eq!("owner0001", response.owner.as_str());

        let res = query(deps.as_ref(), env.clone(), QueryMsg::GetPayments {}).unwrap();
        let payments: PaymentsResponse = from_binary(&res).unwrap();

        let expected_payments = vec![
            PaymentState {
                payment: Payment {
                    recipient: Addr::unchecked("addr0002"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(10u64),
                },
                paid: false,
                id: 1,
            },
        ];

        assert_eq!(payments.payments, expected_payments);

        env.block.height = 11u64;
        let res = query(deps.as_ref(), env.clone(), QueryMsg::GetPayments {}).unwrap();
        let payments: PaymentsResponse = from_binary(&res).unwrap();
        assert_eq!(true, payments.payments[0].payment.time.is_expired(&env.block))
    }

    #[test]
    fn single_cw20_payment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            schedule: vec![
                Payment {
                    recipient: Addr::unchecked("addr0002"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(10u64),
                }
            ],
            owner_address: Some(Addr::unchecked("owner0001".to_string())),
            token_address: Addr::unchecked("kleo00001"),
        };

        let mut env = mock_env();
        let info = mock_info("owner0001", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        env.block.height = 11u64;

        let execute_msg = ExecuteMsg::Pay {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), execute_msg);

        let res = query(deps.as_ref(), env.clone(), QueryMsg::GetPayments {}).unwrap();
        let payments: PaymentsResponse = from_binary(&res).unwrap();
        let expected_payments = vec![
            PaymentState {
                payment: Payment {
                    recipient: Addr::unchecked("addr0002"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(10u64),
                },
                paid: true,
                id: 1,
            },
        ];

        assert_eq!(payments.payments, expected_payments);
    }

    #[test]
    fn revoke_cw20_payment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            schedule: vec![
                Payment {
                    recipient: Addr::unchecked("addr0002"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(10u64),
                },
                Payment {
                    recipient: Addr::unchecked("addr0003"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(3u64),
                }
            ],
            owner_address: Some(Addr::unchecked("owner0001".to_string())),
            token_address: Addr::unchecked("kleo00001"),
        };

        let mut env = mock_env();
        let info = mock_info("owner0001", &[]);

        // we can just call .unwrap() to assert this was a success
        let _ = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // unauthorized delete
        let execute_msg = ExecuteMsg::RevokePayments {
            payment_ids: vec![1u64, 2u64],
        };
        let info = mock_info("addr0000", &[]);
        let err = execute(deps.as_mut(), env.clone(), info.clone(), execute_msg).unwrap_err();

        assert!(matches!(err, ContractError::Unauthorized {}));

        // authorized delete
        env.block.height = 4u64;
        let execute_msg = ExecuteMsg::Pay {};
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), execute_msg);

        let execute_msg = ExecuteMsg::RevokePayments {
            payment_ids: vec![1u64, 2u64],
        };
        let info = mock_info("owner0001", &[]);
        let _ = execute(deps.as_mut(), env.clone(), info.clone(), execute_msg).unwrap();

        let res = query(deps.as_ref(), env.clone(), QueryMsg::GetPayments {}).unwrap();
        let payments: PaymentsResponse = from_binary(&res).unwrap();
        let expected_payments = vec![
            PaymentState {
                payment: Payment {
                    recipient: Addr::unchecked("addr0003"),
                    amount: Uint128::from(100u64),
                    time: Expiration::AtHeight(3u64),
                },
                paid: true,
                id: 2,
            },
        ];

        assert_eq!(payments.payments, expected_payments);
    }
}
